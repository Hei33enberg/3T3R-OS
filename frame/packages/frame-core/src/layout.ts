import { useCallback, useEffect, useState } from 'react';
import { FRAME_LAYOUT, type FrameSide } from './contract';

export interface FrameOpenState {
  left: boolean;
  right: boolean;
}

export interface FrameLayout {
  /** true from FRAME_LAYOUT.laptopFrom px: both panels may stand open beside the center. */
  isLaptop: boolean;
  open: FrameOpenState;
  /** true when a panel covers the center (phone with a panel open). Pause the 3D stage then. */
  centerCovered: boolean;
  toggle: (side: FrameSide) => void;
  close: (side: FrameSide) => void;
}

const laptopQuery = `(min-width: ${FRAME_LAYOUT.laptopFrom}px)`;

function matchesLaptop(): boolean {
  return typeof window === 'undefined' ? true : window.matchMedia(laptopQuery).matches;
}

/** On a phone only one panel may be open. */
function onePanel(open: FrameOpenState): FrameOpenState {
  return open.left && open.right ? { left: false, right: false } : open;
}

/**
 * Panel behaviour of the frame.
 * Laptop: both panels open at start and stand beside the live center; each one opens and closes on its own.
 * Phone: the center shows at start; a panel opens full screen and opening one closes the other.
 */
export function useFrameLayout(initial?: Partial<FrameOpenState>): FrameLayout {
  const [isLaptop, setIsLaptop] = useState(matchesLaptop);
  const [open, setOpen] = useState<FrameOpenState>(() => {
    const start = matchesLaptop() ? { left: true, right: true } : { left: false, right: false };
    const merged = {
      left: initial?.left ?? start.left,
      right: initial?.right ?? start.right,
    };
    return matchesLaptop() ? merged : onePanel(merged);
  });

  useEffect(() => {
    const mq = window.matchMedia(laptopQuery);
    const onChange = () => setIsLaptop(mq.matches);
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);

  useEffect(() => {
    if (!isLaptop) setOpen(onePanel);
  }, [isLaptop]);

  const toggle = useCallback(
    (side: FrameSide) =>
      setOpen((o) => {
        const next = !o[side];
        if (!isLaptop && next) return { left: side === 'left', right: side === 'right' };
        return { ...o, [side]: next };
      }),
    [isLaptop],
  );

  const close = useCallback((side: FrameSide) => setOpen((o) => ({ ...o, [side]: false })), []);

  return { isLaptop, open, centerCovered: !isLaptop && (open.left || open.right), toggle, close };
}
