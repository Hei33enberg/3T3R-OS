import type { ReactNode } from 'react';

/**
 * 33.0S slot contract.
 *
 * The frame owns the layout and the behaviour. A skin (an app) fills the slots with its own
 * sections, look and data. The frame ships no product content, no colours of any brand and
 * no data layer: data comes from the app's own client of the m.0S hub.
 */

export type FrameSide = 'left' | 'right';
export type FrameColumn = FrameSide | 'center';

/** One section of a side panel. Only one section of a panel is visible at a time. */
export interface FrameSection {
  id: string;
  /** Name shown in the section picker. */
  label: string;
  /** Optional counter next to the name. */
  count?: number;
  /** Content of the section. Rendered only while the section is selected. */
  render: () => ReactNode;
}

/** A side panel: bar, section picker, the selected section and a bar pinned to the bottom. */
export interface FramePanel {
  /** Title in the panel bar. */
  title: string;
  /** Counter in the panel bar (for example: notifications of this panel). */
  count?: number;
  /** The panel's mark: the control that opens and closes it. The skin brings its own glyph. */
  mark?: ReactNode;
  /** Sections of the panel. The first one is selected at start. */
  sections: FrameSection[];
  /** Bar pinned to the bottom of the panel, visible with every section (left: dock, right: footer). */
  bottom?: ReactNode;
}

/** One entry of the floating menu of the center. */
export interface FrameMenuItem {
  id: string;
  label: string;
  icon?: ReactNode;
  /** Starts a new group: a hairline above the entry. */
  startsGroup?: boolean;
  /** The entry opens another screen: a chevron is shown. Entries that act in place show none. */
  opensScreen?: boolean;
  onSelect?: () => void;
}

/** One option of the view pill at the top of the center. */
export interface FrameViewOption {
  id: string;
  label: string;
  icon?: ReactNode;
}

export interface FrameCenter {
  /** The live center. Usually <FrameStage> from @33os/frame-view with the skin's scene inside. */
  view: ReactNode;
  /** Options of the pill at the top of the center. Leave empty to hide the pill. */
  views?: FrameViewOption[];
  /** Selected option of the pill (controlled). */
  activeView?: string;
  onViewChange?: (id: string) => void;
  /** Entries of the one floating menu of the center. */
  menu: FrameMenuItem[];
  /** Label of the menu handle. */
  menuLabel?: string;
}

export interface FrameConfig {
  left: FramePanel;
  center: FrameCenter;
  right: FramePanel;
}

export interface FrameSlot {
  id: string;
  column: FrameColumn;
  what: string;
}

/** Every place a skin can fill. Ids are stable; documentation and placeholders use them. */
export const FRAME_SLOTS: readonly FrameSlot[] = [
  { id: 'left.bar', column: 'left', what: 'Panel bar: mark (opens and closes the panel), title, counter.' },
  { id: 'left.sections', column: 'left', what: 'Section picker: the main products of the app, one visible at a time.' },
  { id: 'left.section', column: 'left', what: 'The selected section.' },
  { id: 'left.dock', column: 'left', what: 'Bar pinned to the bottom of the left panel, visible with every section.' },
  { id: 'center.views', column: 'center', what: 'Pill at the top of the center that switches the view.' },
  { id: 'center.view', column: 'center', what: 'The live center: a 3D stage the skin fills with its scene.' },
  { id: 'center.menu', column: 'center', what: 'The one floating menu of the center. No other bars or buttons on the center.' },
  { id: 'right.bar', column: 'right', what: 'Panel bar: mark (opens and closes the panel), title, counter.' },
  { id: 'right.sections', column: 'right', what: 'Section picker of the right panel, one visible at a time.' },
  { id: 'right.section', column: 'right', what: 'The selected section.' },
  { id: 'right.footer', column: 'right', what: 'Footer pinned to the bottom of the right panel.' },
] as const;

/** Layout rules of the frame. */
export const FRAME_LAYOUT = {
  /** From this width both panels can stand open beside the live center. Below it a panel opens full screen, one at a time. */
  laptopFrom: 1024,
  /** Default panel width. Two panels together take at most 78% of the window, the center keeps at least 22%. */
  panelWidth: 'min(clamp(360px, 33.333vw, 720px), 39vw)',
  /** Minimum touch target. */
  touch: 44,
} as const;
