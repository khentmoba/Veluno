# Google Keep startup evidence

`before.png` and `after.png` are the same `fixture.html` page captured from the pre-fix and fixed
debug executables, using an isolated profile and a local synthetic page. No personal browsing data
appears.

`fixture.html` reproduces what Google Keep's instrumentation does at startup: it assigns a global
`ipc` function and calls it. wry's IPC bridge used to define a frozen, non-writable `window.ipc`
object before any page script ran, so Keep's assignment silently failed and the call threw
`ipc is not a function`, leaving the page header painted and the notes area empty.

The pre-fix capture reports `App failed: window.ipc is not a function` with
`typeof window.ipc = object`. The fixed capture reports `App initialised` with
`typeof window.ipc = function`.

The same failure and fix were observed against the real `https://keep.google.com/` page in an
isolated copy of a signed-in profile: before the fix the document held 129 elements, one visible
word (`Keep`), and two uncaught exceptions from the Keep bundle; after the fix it holds 613
elements, a visible sidebar, visible note cards, and no exceptions.
