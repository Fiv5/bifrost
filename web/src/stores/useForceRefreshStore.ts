import { create } from 'zustand';

interface ForceRefreshState {
  visible: boolean;
  reason: string;
  show: (reason: string) => void;
  hide: () => void;
}

export const useForceRefreshStore = create<ForceRefreshState>((set) => ({
  visible: false,
  reason: '',
  show: (reason) => set({ visible: true, reason }),
  hide: () => set({ visible: false, reason: '' }),
}));

