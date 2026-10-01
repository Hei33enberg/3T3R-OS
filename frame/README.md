# 33.0S frame — how to put your app into it

This folder is the **empty 33.0S frame** as code: a left panel, a live 3D center and a right panel, one floating menu and a view pill. It is neutral to brand: grey on black, system type, no product content and no data layer. An app brings its own look (tokens), its own sections and its own scene, and reads its data through its own client of the m.0S hub.

| Laptop (from 1024 px) | Phone |
|---|---|
| Both panels stand open **beside** the live center. Each panel opens and closes on its own. The center keeps at least 22% of the window. | The center shows at start. A panel opens **full screen**, one at a time; opening one closes the other. The 3D stage pauses while a panel covers it. |

## Run it

```bash
cd frame
npm install
npm run dev        # http://localhost:4333
npm run build      # apps/shell/dist — static files, run from any folder or static host
npm run typecheck
```

Preview switches for reviews and screenshots: `?left=0|1&right=0|1&menu=1`.

## What is inside

| Package | What it holds |
|---|---|
| [`packages/frame-core`](packages/frame-core) | The slot contract (`FRAME_SLOTS`, `FrameConfig`), the layout rules (`FRAME_LAYOUT`, `useFrameLayout`), the empty shell (`FrameShell`, `SlotBox`) and the neutral theme (`frame.css`, `--f33-*` tokens). |
| [`packages/frame-view`](packages/frame-view) | `FrameStage`, the live center on three.js / react-three-fiber. Rotate and zoom, no panning; a slow idle flight that stops on touch; frames on demand when idle; stops when the tab is hidden or a panel covers the center; no idle flight under reduced motion. Your scene goes inside as children. |
| [`apps/shell`](apps/shell) | The empty frame with every slot labelled by its name. Copy it to start an app. |

Stack: TypeScript, React 18, Vite 5, three 0.160, @react-three/fiber 8, @react-three/drei 9.

## Slots

| Slot | What goes there |
|---|---|
| `left.bar` | Panel bar: mark (opens and closes the panel), title, counter. |
| `left.sections` | Section picker: the main products of the app, one visible at a time. |
| `left.section` | The selected section. |
| `left.dock` | Bar pinned to the bottom of the left panel, visible with every section. |
| `center.views` | Pill at the top of the center that switches the view. |
| `center.view` | The live center: a 3D stage the app fills with its scene. |
| `center.menu` | The one floating menu of the center. No other bars or buttons on the center. |
| `right.bar` | Panel bar of the right panel. |
| `right.sections` | Section picker of the right panel, one visible at a time. |
| `right.section` | The selected section. |
| `right.footer` | Footer pinned to the bottom of the right panel. |

## Put your app into 33.0S

1. **Skin = tokens.** Override the `--f33-*` variables after importing `@33os/frame-core/frame.css`. Bring your own fonts and glyphs (panel marks, menu icons). Do not restyle the layout.

   ```css
   :root {
     --f33-bg: #000;
     --f33-text: #f5f5f5;
     --f33-focus: #ffffff;            /* your accent */
     --f33-font: "Your Sans", system-ui, sans-serif;
     --f33-radius: 4px;
   }
   ```

2. **Sections = slots.** Describe both panels in a `FrameConfig`. Each panel has `sections` (one visible at a time, each with its own `render()`), an optional `count` and a `bottom` bar (the dock on the left, the footer on the right).

   ```tsx
   const layout = useFrameLayout();
   const config: FrameConfig = {
     left:  { title: 'Ops', sections: [{ id: 'inbox', label: 'Inbox', render: () => <Inbox /> }], bottom: <Dock /> },
     center: { view: <FrameStage paused={layout.centerCovered}><YourScene /></FrameStage>, menu: [...] },
     right: { title: 'Account', sections: [...], bottom: <Footer /> },
   };
   return <FrameShell config={config} layout={layout} />;
   ```

3. **Center = the 3D view.** Put your scene inside `<FrameStage>`. Pass `stage={false}` when your scene brings its own floor and sky. Pass `paused={layout.centerCovered}` so the stage stops while a phone panel covers it.

4. **Menu = one floating menu.** Everything the center offers goes into `config.center.menu`. Mark entries that open another screen with `opensScreen`; entries that act in place show no chevron. Start a group with `startsGroup`.

5. **Data = your client of the m.0S hub** ([mosadd.dev](https://mosadd.dev)). Identity, memory, events and balance come from the hub through your app's own client. The frame has no data layer and never calls a server.

### Example: mosADD (version B) — steps for the agent that moves mosADD into the frame

1. Add `@33os/frame-core` and `@33os/frame-view` to the app and mount `FrameShell` as the main screen.
2. Map the mosADD design tokens to the `--f33-*` variables. The frame keeps no mosADD colours.
3. Left panel: one section per main operational product of the app, one visible at a time; the dock stays at the bottom with every section.
4. Right panel: the account side of the app in sections, with the footer at the bottom.
5. Center: the app's own 3D scene as a child of `FrameStage`; keep panels beside it on a laptop, and pause it when a phone panel covers it.
6. Menu: the app's center entries go into the one floating menu; no new bars or buttons on the center.
7. Data: read everything the screens show through the app's m.0S hub client.
8. Check the result at laptop 1536×864, phone 375×667 and phone 412×915 before calling it done.

## License

Apache-2.0, like the rest of this repository. See [`../LICENSE`](../LICENSE).
