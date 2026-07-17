import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
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
  const [workspaceFocusRequest, setWorkspaceFocusRequest] = useState(0);
  const scrollPositions = useRef(new Map<string, number>());
  const pendingScroll = useRef<number | null | undefined>(undefined);
  const currentPathname = useRef(pathname);

  useEffect(() => {
    if (initialPath !== undefined) return undefined;
    const previousScrollRestoration = globalThis.history.scrollRestoration;
    globalThis.history.scrollRestoration = 'manual';
    const syncFromBrowser = () => {
      const nextPathname = pathnameFrom(globalThis.location.pathname);
      scrollPositions.current.set(currentPathname.current, globalThis.scrollY ?? 0);
      pendingScroll.current = scrollPositions.current.get(nextPathname) ?? null;
      currentPathname.current = nextPathname;
      setPathname(nextPathname);
      setWorkspaceFocusRequest((request) => request + 1);
    };
    globalThis.addEventListener('popstate', syncFromBrowser);
    return () => {
      globalThis.history.scrollRestoration = previousScrollRestoration;
      globalThis.removeEventListener('popstate', syncFromBrowser);
    };
  }, [initialPath]);

  useEffect(() => {
    if (workspaceFocusRequest === 0) return;
    const workspace = globalThis.document?.getElementById('bcsp-workspace');
    if (workspace === null || workspace === undefined) return;
    workspace.focus({ preventScroll: true });
    const restore = pendingScroll.current;
    pendingScroll.current = undefined;
    if (typeof restore === 'number') {
      globalThis.scrollTo?.({ behavior: 'auto', left: 0, top: restore });
      return;
    }
    globalThis.document?.querySelector<HTMLElement>('.bcsp-navigation')
      ?.scrollIntoView?.({ behavior: 'auto', block: 'start' });
  }, [workspaceFocusRequest]);

  const navigate = useCallback((to: string, options?: { readonly replace?: boolean }) => {
    const nextPathname = pathnameFrom(to);
    scrollPositions.current.set(pathname, globalThis.scrollY ?? 0);
    pendingScroll.current = scrollPositions.current.get(nextPathname) ?? null;
    if (initialPath === undefined) {
      if (options?.replace === true) globalThis.history.replaceState(null, '', to);
      else globalThis.history.pushState(null, '', to);
    }
    setPathname(nextPathname);
    currentPathname.current = nextPathname;
    setWorkspaceFocusRequest((request) => request + 1);
  }, [initialPath, pathname]);

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
