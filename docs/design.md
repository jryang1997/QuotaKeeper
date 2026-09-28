# Token Monitor UI design

## Direction

Token Monitor should feel like a calm instrument panel: the current picture is easy to scan, important numbers carry the visual weight, and controls stay close to the information they change. Use restrained color, clear grouping, and short labels. Motion should explain a change, not decorate an idle screen.

## Type

- Use Segoe UI Variable for Latin text and numbers, and Microsoft YaHei UI for Chinese UI text by default. Give key numbers tabular alignment and enough size to scan.
- Sixtyfour and 秋叶圆体 16 remain an optional pixel style. In the standard style, summary values use clean typographic numerals instead of the 5×7 dot matrix.
- Keep supporting labels at 12px or larger and use the color and spacing hierarchy to distinguish data instead of relying on decorative lettering.

Font files and their SIL Open Font Licenses are kept in `gui/frontend/public/fonts/`.

## Preferences

The Settings page owns language, system/light/dark/midnight/warm-paper appearance, typeface, comfortable/compact density, reduced motion, desktop notifications, quiet hours, and the monthly plan benchmark. Preferences are stored locally in the desktop web view. The monthly benchmark continues to use the app's settings file so both interfaces share the same value. The current theme is also available in the main toolbar and through `P`.

System appearance follows Windows. System motion follows the operating system's reduced-motion preference. Changes apply immediately and remain available the next time the app opens.

The Windows shell keeps a tray icon while the window is hidden. A second launch restores the existing window. Desktop notifications use the shared 80%/100%/reset transition rules and ignore CPA activity intensity. The first successful quota read is the baseline; the optional quiet period is 22:00–08:00 local time.

## Layout and feedback

- Keep quota, usage, and heatmap as primary destinations; Settings is a separate destination.
- Put related controls in small, named sections with the current value visibly selected.
- Keep the dashboard focused on usage. Edit the plan benchmark in Settings and retain a direct link from its dashboard summary.
- Use brief fades and small position changes between pages. Reduced motion removes movement and shortens transitions.
- Keep controls keyboard reachable and provide visible focus rings.
- Give service quotas and local usage independent freshness indicators. Suppress pace estimates when quota data is stale or the window sample is too short; label estimates as linear extrapolations.
- Surface source diagnostics on demand. Local CSV and JSON export is initiated from the usage page through a Windows save dialog.
