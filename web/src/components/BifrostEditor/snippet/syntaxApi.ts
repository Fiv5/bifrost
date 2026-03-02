export interface ProtocolInfo {
  name: string;
  category: string;
  description: string;
  value_type: string;
  example: string;
  alias_of?: string;
  aliases: string[];
}

export interface TemplateVariableInfo {
  name: string;
  description: string;
  example: string;
  category: string;
}

export interface PatternInfo {
  name: string;
  description: string;
  example: string;
}

export interface ScriptListItem {
  name: string;
  description?: string;
}

export interface ScriptsInfo {
  request_scripts: ScriptListItem[];
  response_scripts: ScriptListItem[];
}

export interface ValueHint {
  prefix: string;
  description: string;
  example: string;
  completions: string[];
}

export interface ProtocolValueSpec {
  protocol: string;
  value_format: string;
  hints: ValueHint[];
  validation_regex?: string;
}

export interface UnifiedSyntaxInfo {
  protocols: ProtocolInfo[];
  template_variables: TemplateVariableInfo[];
  patterns: PatternInfo[];
  protocol_aliases: Record<string, string>;
  scripts: ScriptsInfo;
  filter_specs: ProtocolValueSpec[];
}

let cachedSyntaxInfo: UnifiedSyntaxInfo | null = null;
let fetchPromise: Promise<UnifiedSyntaxInfo> | null = null;

export async function fetchSyntaxInfo(): Promise<UnifiedSyntaxInfo> {
  if (cachedSyntaxInfo) {
    return cachedSyntaxInfo;
  }

  if (fetchPromise) {
    return fetchPromise;
  }

  fetchPromise = fetch('/_bifrost/api/syntax')
    .then((response) => {
      if (!response.ok) {
        throw new Error(`Failed to fetch syntax info: ${response.status}`);
      }
      return response.json();
    })
    .then((data: UnifiedSyntaxInfo) => {
      cachedSyntaxInfo = data;
      return data;
    })
    .finally(() => {
      fetchPromise = null;
    });

  return fetchPromise;
}

export function getCachedSyntaxInfo(): UnifiedSyntaxInfo | null {
  return cachedSyntaxInfo;
}

export function clearSyntaxCache(): void {
  cachedSyntaxInfo = null;
}
