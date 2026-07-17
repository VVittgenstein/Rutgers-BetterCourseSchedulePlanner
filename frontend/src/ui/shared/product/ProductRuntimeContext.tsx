import { createContext, useContext, type ReactNode } from 'react';

import type { ProductBootstrapFailureReason } from './bootstrap';
import type { ProductRuntimePort } from './runtime';

export type ProductRuntimeState =
  | { readonly status: 'LOADING' }
  | { readonly status: 'READY'; readonly runtime: ProductRuntimePort }
  | { readonly status: 'ERROR'; readonly reason: ProductBootstrapFailureReason };

export const PRODUCT_RUNTIME_LOADING: ProductRuntimeState = { status: 'LOADING' };

const ProductRuntimeContext = createContext<ProductRuntimeState | null>(null);

export interface ProductRuntimeProviderProps {
  readonly children: ReactNode;
  readonly state: ProductRuntimeState;
}

export function ProductRuntimeProvider({ children, state }: ProductRuntimeProviderProps) {
  return (
    <ProductRuntimeContext.Provider value={state}>
      {children}
    </ProductRuntimeContext.Provider>
  );
}

export function useProductRuntimeState(): ProductRuntimeState {
  const state = useContext(ProductRuntimeContext);
  if (state === null) throw new Error('Product runtime provider is missing');
  return state;
}
