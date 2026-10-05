# Web persistence

The web app may cache versioned application shell files so the UI can open offline. Encrypted vault records stay in the IndexedDB adapter. A service worker must not cache vault responses, message bodies, or signer traffic.

The browser can suspend or close the tab. Catch-up then waits until the app is open again. A closed browser is not a background sync guarantee. Multi-tab writes keep the existing single Web Lock. A downloaded archive file does not update itself when the vault changes.
