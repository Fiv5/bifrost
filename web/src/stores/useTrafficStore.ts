import { create } from 'zustand';
import type { TrafficSummary, TrafficRecord, TrafficFilter, ToolbarFilters, FilterCondition } from '../types';
import * as api from '../api';

interface TrafficState {
  records: TrafficSummary[];
  currentRecord: TrafficRecord | null;
  requestBody: string | null;
  responseBody: string | null;
  total: number;
  filter: TrafficFilter;
  toolbarFilters: ToolbarFilters;
  filterConditions: FilterCondition[];
  paused: boolean;
  loading: boolean;
  error: string | null;
  fetchTraffic: () => Promise<void>;
  fetchTrafficDetail: (id: string) => Promise<void>;
  clearTraffic: () => Promise<boolean>;
  setFilter: (filter: Partial<TrafficFilter>) => void;
  setToolbarFilters: (filters: ToolbarFilters) => void;
  setFilterConditions: (conditions: FilterCondition[]) => void;
  setPaused: (paused: boolean) => void;
  clearError: () => void;
  clearCurrentRecord: () => void;
}

const buildFilterFromToolbar = (toolbar: ToolbarFilters, conditions: FilterCondition[]): Partial<TrafficFilter> => {
  const filter: Partial<TrafficFilter> = {};
  
  if (toolbar.rule.includes('Hit Rule')) {
    filter.has_rule_hit = true;
  }

  if (toolbar.protocol.length > 0) {
    const protocols = toolbar.protocol.map(p => p.toLowerCase());
    if (protocols.length === 1) {
      filter.protocol = protocols[0];
    }
  }

  if (toolbar.status.length > 0) {
    const statusRanges: { min: number; max: number }[] = [];
    toolbar.status.forEach(s => {
      if (s === '1xx') statusRanges.push({ min: 100, max: 199 });
      else if (s === '2xx') statusRanges.push({ min: 200, max: 299 });
      else if (s === '3xx') statusRanges.push({ min: 300, max: 399 });
      else if (s === '4xx') statusRanges.push({ min: 400, max: 499 });
      else if (s === '5xx') statusRanges.push({ min: 500, max: 599 });
    });
    if (statusRanges.length === 1) {
      filter.status_min = statusRanges[0].min;
      filter.status_max = statusRanges[0].max;
    }
  }

  if (toolbar.type.length > 0) {
    const typeMap: Record<string, string> = {
      'JSON': 'json',
      'Form': 'form',
      'XML': 'xml',
      'JS': 'javascript',
      'CSS': 'css',
      'Font': 'font',
      'Doc': 'html',
      'Media': 'image',
    };
    const types = toolbar.type.map(t => typeMap[t] || t.toLowerCase());
    if (types.length === 1) {
      filter.content_type = types[0];
    }
  }

  conditions.forEach(cond => {
    if (!cond.value) return;
    const value = cond.value;
    switch (cond.field) {
      case 'url':
        filter.url_contains = value;
        break;
      case 'host':
        filter.host = value;
        break;
      case 'path':
        filter.path_contains = value;
        break;
      case 'method':
        filter.method = value.toUpperCase();
        break;
      case 'content_type':
        filter.content_type = value;
        break;
      case 'request_header':
      case 'response_header':
        filter.header_contains = value;
        break;
      case 'domain':
        filter.domain = value;
        break;
    }
  });

  return filter;
};

export const useTrafficStore = create<TrafficState>((set, get) => ({
  records: [],
  currentRecord: null,
  requestBody: null,
  responseBody: null,
  total: 0,
  filter: { limit: 500, offset: 0 },
  toolbarFilters: { rule: [], protocol: [], type: [], status: [] },
  filterConditions: [],
  paused: false,
  loading: false,
  error: null,

  fetchTraffic: async () => {
    if (get().paused) return;
    
    set({ loading: true, error: null });
    try {
      const state = get();
      const toolbarFilter = buildFilterFromToolbar(state.toolbarFilters, state.filterConditions);
      const mergedFilter = { ...state.filter, ...toolbarFilter };
      const response = await api.getTrafficList(mergedFilter);
      set({ 
        records: response.records, 
        total: response.total, 
        loading: false 
      });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  fetchTrafficDetail: async (id: string) => {
    set({ loading: true, error: null, requestBody: null, responseBody: null });
    try {
      const record = await api.getTrafficDetail(id);
      set({ currentRecord: record, loading: false });
      
      api.getRequestBody(id).then(body => {
        set({ requestBody: body });
      }).catch(() => {});
      
      api.getResponseBody(id).then(body => {
        set({ responseBody: body });
      }).catch(() => {});
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  clearTraffic: async () => {
    set({ loading: true, error: null });
    try {
      await api.clearTraffic();
      set({ 
        records: [], 
        total: 0, 
        currentRecord: null, 
        requestBody: null,
        responseBody: null,
        loading: false 
      });
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  setFilter: (filter: Partial<TrafficFilter>) => {
    set((state) => ({
      filter: { ...state.filter, ...filter },
    }));
  },

  setToolbarFilters: (filters: ToolbarFilters) => {
    set({ toolbarFilters: filters });
    get().fetchTraffic();
  },

  setFilterConditions: (conditions: FilterCondition[]) => {
    set({ filterConditions: conditions });
  },

  setPaused: (paused: boolean) => {
    set({ paused });
  },

  clearError: () => set({ error: null }),

  clearCurrentRecord: () => set({ 
    currentRecord: null, 
    requestBody: null, 
    responseBody: null 
  }),
}));
