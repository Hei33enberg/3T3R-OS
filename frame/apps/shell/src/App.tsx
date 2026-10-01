import { useState } from 'react';
import { FrameShell, SlotBox, useFrameLayout, type FrameConfig, type FrameSection } from '@33os/frame-core';
import { FrameStage } from '@33os/frame-view';

/**
 * The empty 33.0S frame. Every slot is filled with a placeholder that shows the slot's name,
 * so an app team can see where its content goes. Replace the placeholders with your app.
 *
 * Preview switches (for screenshots and reviews): ?left=0|1 &right=0|1 &menu=1
 */
const params = new URLSearchParams(window.location.search);
const flag = (name: string): boolean | undefined => (params.has(name) ? params.get(name) === '1' : undefined);

function emptySections(side: 'left' | 'right'): FrameSection[] {
  return [1, 2, 3].map((n) => ({
    id: `${side}-${n}`,
    label: `Section ${n}`,
    render: () => <SlotBox slot={`${side}.section`} note={`Section ${n}`} grow />,
  }));
}

export function App() {
  const layout = useFrameLayout({ left: flag('left'), right: flag('right') });
  const [view, setView] = useState('a');

  const config: FrameConfig = {
    left: {
      title: 'Left',
      sections: emptySections('left'),
      bottom: <SlotBox slot="left.dock" />,
    },
    center: {
      view: <FrameStage paused={layout.centerCovered} />,
      views: [
        { id: 'a', label: 'View A' },
        { id: 'b', label: 'View B' },
      ],
      activeView: view,
      onViewChange: setView,
      menu: [
        { id: 'm1', label: 'Menu item 1', opensScreen: true },
        { id: 'm2', label: 'Menu item 2', opensScreen: true },
        { id: 'm3', label: 'Menu item 3', opensScreen: true },
        { id: 'm4', label: 'Menu item 4', startsGroup: true },
      ],
    },
    right: {
      title: 'Right',
      sections: emptySections('right'),
      bottom: <SlotBox slot="right.footer" />,
    },
  };

  return (
    <FrameShell
      config={config}
      layout={layout}
      initialMenuOpen={flag('menu')}
      centerOverlay={
        <div className="shell-center-label" aria-hidden>
          <b>center.view</b>
          <span>The live center — the skin puts its scene here</span>
        </div>
      }
    />
  );
}
