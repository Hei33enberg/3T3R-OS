import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { Canvas } from '@react-three/fiber';
import { Grid, OrbitControls } from '@react-three/drei';
import { seeded, useReducedMotion, useTabHidden } from './guards';

export interface FrameStageProps {
  /** Stop drawing, e.g. while a phone panel covers the center. */
  paused?: boolean;
  /** Show the neutral stage (floor grid and point field). Skins with their own world can turn it off. */
  stage?: boolean;
  /** Idle flight speed around the center (OrbitControls autoRotate units). 0 = no idle flight. */
  idleSpeed?: number;
  /** Seconds after the last touch before the idle flight resumes. */
  resumeAfter?: number;
  /** The skin's scene. */
  children?: ReactNode;
}

/** Neutral greys. A skin that wants colour brings its own scene. */
const GRID_CELL = '#26262b';
const GRID_SECTION = '#3a3a41';
const POINTS = '#a1a1aa';

function PointField({ count = 900 }: { count?: number }) {
  const positions = useMemo(() => {
    const rnd = seeded(33);
    const out = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
      // a thick shell far around the stage (beyond the camera's reach), kept above the floor
      const r = 26 + rnd() * 18;
      const theta = rnd() * Math.PI * 2;
      const y = rnd() * 0.9 + 0.05;
      const ring = Math.sqrt(1 - y * y);
      out[i * 3] = Math.cos(theta) * ring * r;
      out[i * 3 + 1] = y * r - 1;
      out[i * 3 + 2] = Math.sin(theta) * ring * r;
    }
    return out;
  }, [count]);

  return (
    <points>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
      </bufferGeometry>
      <pointsMaterial color={POINTS} size={1.6} sizeAttenuation={false} transparent opacity={0.75} depthWrite={false} />
    </points>
  );
}

function NeutralStage() {
  return (
    <>
      <Grid
        position={[0, -1, 0]}
        infiniteGrid
        cellSize={0.5}
        cellThickness={0.6}
        cellColor={GRID_CELL}
        sectionSize={2.5}
        sectionThickness={1}
        sectionColor={GRID_SECTION}
        fadeDistance={26}
        fadeStrength={1.4}
      />
      <PointField />
    </>
  );
}

/**
 * The live center of 33.0S: a 3D stage that stays alive beside open panels.
 * - The user rotates and zooms (no panning). An idle flight circles the center and stops on touch.
 * - Draws continuously only while flying; otherwise frames are drawn on demand.
 * - Stops when paused, when the tab is hidden, and flies never under reduced motion.
 * - The canvas is transparent: the frame background (--f33-bg) shows through.
 */
export function FrameStage({ paused = false, stage = true, idleSpeed = 0.35, resumeAfter = 8, children }: FrameStageProps) {
  const hidden = useTabHidden();
  const reduced = useReducedMotion();
  const [touched, setTouched] = useState(false);
  const resumeTimer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(resumeTimer.current), []);

  const live = !paused && !hidden;
  const flying = live && !reduced && !touched && idleSpeed > 0;

  return (
    <Canvas
      frameloop={!live ? 'never' : flying ? 'always' : 'demand'}
      dpr={[1, 1.75]}
      camera={{ position: [0, 2.6, 8], fov: 50, near: 0.1, far: 200 }}
      gl={{ alpha: true, antialias: true, powerPreference: 'high-performance' }}
      onCreated={({ gl }) => {
        // keep the page alive if the GPU drops the context; the browser restores it
        gl.domElement.addEventListener('webglcontextlost', (e) => e.preventDefault());
      }}
      style={{ position: 'absolute', inset: 0 }}
    >
      {stage && <NeutralStage />}
      {children}
      <OrbitControls
        makeDefault
        enablePan={false}
        enableDamping={!reduced}
        minDistance={3}
        maxDistance={20}
        maxPolarAngle={Math.PI * 0.49}
        autoRotate={flying}
        autoRotateSpeed={idleSpeed}
        onStart={() => {
          window.clearTimeout(resumeTimer.current);
          setTouched(true);
        }}
        onEnd={() => {
          window.clearTimeout(resumeTimer.current);
          resumeTimer.current = window.setTimeout(() => setTouched(false), resumeAfter * 1000);
        }}
      />
    </Canvas>
  );
}
