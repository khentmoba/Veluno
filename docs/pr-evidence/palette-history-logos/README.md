# Palette history site logos evidence

`before.png` captures the search palette when history suggestions omitted `entry.favicon`. For `https://www.experientiallabs.ai/`, where no open tab or shortcut shared the hostname and `/favicon.ico` did not exist (or returned 404), the row fell back to the clock icon while other domains with open tabs displayed site logos.

`after.png` captures the same synthetic fixture in an isolated WebView2 profile after preserving `entry.favicon` on history suggestions in `Veluno.suggestions` and looking up known favicons across history entries with fallback to site root icons. Both entries display their saved site logos consistently.
