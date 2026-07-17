export const frontendTargets = {
  local: {
    devPort: 5174,
    html: 'local.html',
    outDir: 'dist/local',
  },
  public: {
    devPort: 5175,
    html: 'public.html',
    outDir: 'dist/public',
  },
} as const;

export type FrontendTarget = keyof typeof frontendTargets;

export function getTargetBuild(target: FrontendTarget) {
  return frontendTargets[target];
}
