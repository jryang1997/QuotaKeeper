# Bundled fonts

Every typeface option in Settings is a complete EN+CN suite. Files load with
`local()` first (use the OS-installed font when present), then fall back to the
bundled woff2 so a fresh install still renders correctly.

| Suite (data-typeface) | Body / UI | Code / labels | Files | License |
| --- | --- | --- | --- | --- |
| `sarasa` (default) | Sarasa UI SC | Sarasa Term SC | `sarasa-ui-sc-*.woff2`, `sarasa-term-sc-*.woff2` | `OFL-SarasaGothic.txt` |
| `maple` | Maple Mono CN | Maple Mono CN | `maple-mono-cn-*.woff2` | `OFL-MapleMono.txt` |
| `plex` | IBM Plex Sans SC | IBM Plex Mono | `IBMPlexSansSC-*.woff2`, `IBMPlexMono-*.woff2` | `OFL-IBMPlexSansSC.txt`, `OFL-IBMPlexMono.txt` |
| `misans` | MiSans (local detection only) | IBM Plex Mono | none — MiSans's license forbids redistributing the font files | — |
| `smiley` | Smiley Sans | IBM Plex Mono | `smiley-sans-oblique.woff2` | `OFL-SmileySans.txt` |
| `pixel` | Sixtyfour + 秋叶圆体16 | Sixtyfour | `sixtyfour-variable.woff2`, `qiu-ye-yuan-ti-16.ttf` | `OFL-Sixtyfour.txt`, `OFL-QiuYeYuanTi-16.txt` |

Sources:

- Sarasa UI/Term SC: [be5invis/Sarasa-Gothic](https://github.com/be5invis/Sarasa-Gothic) v1.0.42 (UI unhinted; Term converted from the locally hinted TTFs), converted to woff2.
- Maple Mono CN: [subframe7536/maple-font](https://github.com/subframe7536/maple-font) v7.9, Regular + Bold converted to woff2.
- IBM Plex Sans SC Medium/Bold: [@ibm/plex-sans-sc](https://www.npmjs.com/package/@ibm/plex-sans-sc) v1.1.0 complete hinted woff2. Regular/SemiBold predate this and came from [IBM/plex](https://github.com/IBM/plex/tree/master/packages/plex-sans-sc).
- Smiley Sans: [atelier-anchor/smiley-sans](https://github.com/atelier-anchor/smiley-sans) v2.0.1, official woff2.
- `sixtyfour-variable.woff2` comes from [jenskutilek/homecomputer-fonts](https://github.com/jenskutilek/homecomputer-fonts), the Sixtyfour variable font.
- `qiu-ye-yuan-ti-16.ttf` comes from [yzdnn/QiuYeYuanTi-16](https://github.com/yzdnn/QiuYeYuanTi-16), 秋叶圆体 16.

Keep the accompanying licenses with the font files when redistributing the app.
