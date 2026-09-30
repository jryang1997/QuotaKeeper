# Bundled fonts

Two typeface options in Settings, each a complete EN+CN suite. Files load with
`local()` first (use the OS-installed font when present), then fall back to the
bundled woff2 so a fresh install still renders correctly.

| Suite (data-typeface) | Body / UI | Code / labels | Files | License |
| --- | --- | --- | --- | --- |
| `sarasa` (default) | Sarasa UI SC | Sarasa Term SC | `sarasa-ui-sc-*.woff2`, `sarasa-term-sc-*.woff2` | `OFL-SarasaGothic.txt` |
| `plex` | IBM Plex Sans SC | IBM Plex Mono | `IBMPlexSansSC-*.woff2`, `IBMPlexMono-*.woff2` | `OFL-IBMPlexSansSC.txt`, `OFL-IBMPlexMono.txt` |

Sources:

- Sarasa UI/Term SC: [be5invis/Sarasa-Gothic](https://github.com/be5invis/Sarasa-Gothic) v1.0.42 (UI unhinted; Term converted from the locally hinted TTFs), converted to woff2.
- IBM Plex Sans SC: [IBM/plex](https://github.com/IBM/plex/tree/master/packages/plex-sans-sc); Medium/Bold came from the [@ibm/plex-sans-sc](https://www.npmjs.com/package/@ibm/plex-sans-sc) v1.1.0 complete hinted woff2.
- IBM Plex Mono: [IBM/plex](https://github.com/IBM/plex/tree/master/packages/plex-mono).

Keep the accompanying licenses with the font files when redistributing the app.
