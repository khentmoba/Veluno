use crate::{
    profile,
    state::{BrowserState, Tab},
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Page {
    pub id: usize,
    pub generation: u64,
    pub url: String,
    navigation: u64,
}

#[derive(Default)]
pub struct Recovery {
    navigations: HashMap<(usize, u64), u64>,
    failed: HashSet<Page>,
    pending: HashSet<Page>,
    retried: HashSet<(usize, u64, String)>,
}

impl Recovery {
    fn page(&self, tab: &Tab) -> Option<Page> {
        if tab.pending_url.is_some() || profile::web_origin(&tab.url).is_none() {
            return None;
        }
        Some(Page {
            id: tab.id,
            generation: tab.generation,
            url: tab.url.clone(),
            navigation: self
                .navigations
                .get(&(tab.id, tab.generation))
                .copied()
                .unwrap_or(0),
        })
    }

    pub fn navigation_started(&mut self, id: usize, generation: u64) {
        *self.navigations.entry((id, generation)).or_default() += 1;
        self.failed
            .retain(|page| page.id != id || page.generation != generation);
        self.pending
            .retain(|page| page.id != id || page.generation != generation);
    }

    pub fn navigation_completed(&mut self, id: usize, generation: u64) {
        self.retried
            .retain(|(tab_id, tab_gen, _)| *tab_id != id || *tab_gen != generation);
    }

    pub fn incomplete_load(&mut self, tab: &Tab) -> Option<Page> {
        let page = self.page(tab)?;
        if self
            .retried
            .insert((page.id, page.generation, page.url.clone()))
        {
            self.begin(tab)
        } else {
            self.failed.insert(page);
            None
        }
    }

    pub fn script_failed(&mut self, tab: &Tab, document: &str, resource: &str) -> bool {
        let Some(page) = self.page(tab) else {
            return false;
        };
        if document != page.url || profile::web_origin(resource) != profile::web_origin(document) {
            return false;
        }
        self.failed.insert(page)
    }

    pub fn needs_reload(&self, tab: &Tab) -> bool {
        self.page(tab)
            .is_some_and(|page| self.failed.contains(&page))
    }

    pub fn begin(&mut self, tab: &Tab) -> Option<Page> {
        if tab.is_loading {
            return None;
        }
        let page = self.page(tab)?;
        self.pending.insert(page.clone()).then_some(page)
    }

    pub fn finish(&mut self, page: &Page, tab: Option<&Tab>) -> bool {
        self.pending.remove(page)
            && tab.is_some_and(|tab| !tab.is_loading)
            && self.is_current(page, tab)
    }

    pub fn is_current(&self, page: &Page, tab: Option<&Tab>) -> bool {
        tab.is_some_and(|tab| self.page(tab).as_ref() == Some(page))
    }

    pub fn retain(&mut self, state: &BrowserState) {
        let live: HashSet<_> = state
            .tabs
            .iter()
            .filter(|tab| !tab.is_sleeping)
            .map(|tab| (tab.id, tab.generation))
            .collect();
        self.navigations.retain(|key, _| live.contains(key));
        self.failed
            .retain(|page| live.contains(&(page.id, page.generation)));
        self.pending
            .retain(|page| live.contains(&(page.id, page.generation)));
        self.retried
            .retain(|(id, generation, _)| live.contains(&(*id, *generation)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab() -> Tab {
        let mut state = BrowserState::new();
        let tab = &mut state.tabs[0];
        tab.url = "https://example.com/meeting".into();
        tab.generation = 7;
        tab.clone()
    }

    #[test]
    fn reports_are_scoped_to_the_current_document_and_same_origin_scripts() {
        let tab = tab();
        let mut recovery = Recovery::default();
        assert!(!recovery.script_failed(&tab, "https://other.test/", "https://other.test/app.js"));
        assert!(!recovery.script_failed(&tab, &tab.url, "https://tracker.test/ad.js"));
        assert!(recovery.script_failed(&tab, &tab.url, "https://example.com/app.js"));
        assert!(recovery.needs_reload(&tab));
        recovery.navigation_started(tab.id, tab.generation);
        assert!(!recovery.needs_reload(&tab));
    }

    #[test]
    fn refresh_completion_cannot_reload_a_later_visit_to_the_same_url() {
        let tab = tab();
        let mut recovery = Recovery::default();
        let request = recovery.begin(&tab).unwrap();
        recovery.navigation_started(tab.id, tab.generation);
        recovery.navigation_started(tab.id, tab.generation);
        assert!(!recovery.finish(&request, Some(&tab)));
    }

    #[test]
    fn refreshes_are_deduplicated_and_stale_tabs_are_ignored() {
        let mut tab = tab();
        let mut recovery = Recovery::default();
        let request = recovery.begin(&tab).unwrap();
        assert!(recovery.begin(&tab).is_none());
        tab.generation += 1;
        assert!(!recovery.finish(&request, Some(&tab)));
        let request = recovery.begin(&tab).unwrap();
        assert!(recovery.finish(&request, Some(&tab)));
        assert!(!recovery.finish(&request, Some(&tab)));
    }

    #[test]
    fn incomplete_loads_retry_once_then_mark_failed_until_completed() {
        let mut tab = tab();
        let mut recovery = Recovery::default();
        recovery.navigation_started(tab.id, tab.generation);
        let first = recovery.incomplete_load(&tab).unwrap();
        assert!(recovery.finish(&first, Some(&tab)));
        recovery.navigation_started(tab.id, tab.generation);
        assert!(recovery.incomplete_load(&tab).is_none());
        assert!(recovery.needs_reload(&tab));
        tab.error = Some("This page did not finish loading.".into());
        assert!(recovery.needs_reload(&tab));
        recovery.navigation_completed(tab.id, tab.generation);
        recovery.navigation_started(tab.id, tab.generation);
        assert!(recovery.incomplete_load(&tab).is_some());
    }
}
