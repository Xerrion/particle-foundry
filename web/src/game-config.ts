/**
 * Default settings for the active browser sandbox.
 * Physics units and material definitions remain in their owning modules.
 */
export const gameConfig = {
  grid: {
    width: 480,
    height: 270,
  },
  canvas: {
    pixelScale: 2,
  },
  camera: {
    initialZoom: 2,
    initialAnchor: {
      x: 0.5,
      y: 1,
    },
  },
  brush: {
    defaultRadius: 4,
  },
  simulation: {
    defaultSpeed: 1,
    speedOptions: [0.5, 1, 2, 4],
    maxPhysicsBudgetMs: 24,
    initialStepEstimateMs: 8,
    boundariesEnabled: true,
    surfaceShimmerEnabled: true,
  },
} as const;
