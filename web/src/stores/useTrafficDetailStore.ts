import { create } from 'zustand';
import type { SessionTargetSearchState, DisplayFormat } from '../types';
import { DisplayFormat as DF } from '../types';

interface TrafficDetailState {
  requestSearch: SessionTargetSearchState;
  responseSearch: SessionTargetSearchState;
  requestDisplayFormat: DisplayFormat;
  responseDisplayFormat: DisplayFormat;
  requestTab: string;
  responseTab: string;

  setRequestSearch: (v: Partial<SessionTargetSearchState>) => void;
  setResponseSearch: (v: Partial<SessionTargetSearchState>) => void;
  setRequestDisplayFormat: (format: DisplayFormat) => void;
  setResponseDisplayFormat: (format: DisplayFormat) => void;
  setRequestTab: (tab: string) => void;
  setResponseTab: (tab: string) => void;
  reset: () => void;
}

const initialSearchState: SessionTargetSearchState = {
  value: undefined,
  show: false,
  total: 0,
  next: 1,
};

export const useTrafficDetailStore = create<TrafficDetailState>((set) => ({
  requestSearch: initialSearchState,
  responseSearch: initialSearchState,
  requestDisplayFormat: DF.HighLight,
  responseDisplayFormat: DF.HighLight,
  requestTab: 'Overview',
  responseTab: 'Header',

  setRequestSearch: (v) =>
    set((state) => ({
      requestSearch: { ...state.requestSearch, ...v },
    })),
  setResponseSearch: (v) =>
    set((state) => ({
      responseSearch: { ...state.responseSearch, ...v },
    })),
  setRequestDisplayFormat: (format) => set({ requestDisplayFormat: format }),
  setResponseDisplayFormat: (format) => set({ responseDisplayFormat: format }),
  setRequestTab: (tab) => set({ requestTab: tab }),
  setResponseTab: (tab) => set({ responseTab: tab }),
  reset: () =>
    set({
      requestSearch: initialSearchState,
      responseSearch: initialSearchState,
      requestTab: 'Overview',
      responseTab: 'Header',
    }),
}));
