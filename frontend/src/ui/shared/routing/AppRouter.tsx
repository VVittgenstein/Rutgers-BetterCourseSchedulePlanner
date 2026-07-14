import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type AnchorHTMLAttributes,
  type MouseEvent,
  type ReactNode,
} from 'react';

export interface AppRouterRuntime {
  readonly navigate: (to: string, options?: { readonly replace?: boolean }) => void;
  readonly pathname: string;
}

const AppRouterContext = createContext<AppRouterRuntime | null>(null);

export interface AppRouterProviderProps {
  readonly children: ReactNode;
  readonly initialPath?: string;
}

function pathnameFrom(to: string): string {
  return new URL(to, globalThis.location?.href ?? 'http://localhost/').pathname;
}

export function AppRouterProvider({ children, initialPath }: AppRouterProviderProps) {
  const [pathname, setPathname] = useState(() =>
    pathnameFrom(initialPath ?? globalThis.location?.pathname ?? '/'));

  useEffect(() => {
    if (initialPath !== undefined) return undefined;
    const syncFromBrowser = () => setPathname(pathnameFrom(globalThis.location.pathname));
    globalThis.addEventListener('popstate', syncFromBrowser);
    return () => globalThis.removeEventListener('popstate', syncFromBrowser);
  }, [initialPath]);

  const navigate = useCallback((to: string, options?: { readonly replace?: boolean }) => {
    const nextPathname = pathnameFrom(to);
    if (initialPath === undefined) {
      if (options?.replace === true) globalThis.history.replaceState(null, '', to);
      else globalThis.history.pushState(null, '', to);
    }
    setPathname(nextPathname);
  }, [initialPath]);

  const value = useMemo<AppRouterRuntime>(() => ({ navigate, pathname }), [navigate, pathname]);
  return <AppRouterContext.Provider value={value}>{children}</AppRouterContext.Provider>;
}

export function useAppRouter(): AppRouterRuntime {
  const runtime = useContext(AppRouterContext);
  if (runtime === null) throw new Error('AppRouterProvider is missing');
  return runtime;
}

export interface RouterLinkProps extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'href'> {
  readonly to: string;
}

export function RouterLink({ children, onClick, target, to, ...props }: RouterLinkProps) {
  const { navigate } = useAppRouter();

  function follow(event: MouseEvent<HTMLAnchorElement>) {
    onClick?.(event);
    if (
      event.defaultPrevented
      || event.button !== 0
      || event.metaKey
      || event.ctrlKey
      || event.shiftKey
      || event.altKey
      || (target !== undefined && target !== '_self')
    ) return;
    event.preventDefault();
    navigate(to);
  }

  return <a {...props} href={to} onClick={follow} target={target}>{children}</a>;
}
