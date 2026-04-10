import type { RuleFile } from '../../../types';

export type RuleTreeNode = RuleTreeFolderNode | RuleTreeLeafNode;

export interface RuleTreeFolderNode {
  type: 'folder';
  name: string;
  path: string;
  children: RuleTreeNode[];
}

export interface RuleTreeLeafNode {
  type: 'rule';
  name: string;
  label: string;
  rule: RuleFile;
}

export function splitRulePath(name: string): string[] {
  return name
    .split('/')
    .map((segment) => segment.trim())
    .filter((segment) => segment.length > 0);
}

export function getRuleParentPaths(ruleName: string): string[] {
  const segments = splitRulePath(ruleName);
  if (segments.length <= 1) return [];

  const parents: string[] = [];
  let current = '';
  for (const segment of segments.slice(0, -1)) {
    current = current ? `${current}/${segment}` : segment;
    parents.push(current);
  }
  return parents;
}

export function buildRuleTree(rules: RuleFile[]): RuleTreeFolderNode {
  const root: RuleTreeFolderNode = {
    type: 'folder',
    name: '',
    path: '',
    children: [],
  };

  const folderIndex = new Map<string, RuleTreeFolderNode>();

  const getOrCreateFolder = (parent: RuleTreeFolderNode, path: string, name: string) => {
    const existing = folderIndex.get(path);
    if (existing) return existing;

    const created: RuleTreeFolderNode = {
      type: 'folder',
      name,
      path,
      children: [],
    };
    folderIndex.set(path, created);
    parent.children.push(created);
    return created;
  };

  for (const rule of rules) {
    const segments = splitRulePath(rule.name);

    if (segments.length <= 1) {
      const label = segments[0] ?? rule.name;
      root.children.push({ type: 'rule', name: rule.name, label, rule });
      continue;
    }

    let currentFolder = root;
    let currentPath = '';

    for (const [index, segment] of segments.entries()) {
      const isLeaf = index === segments.length - 1;
      if (isLeaf) {
        currentFolder.children.push({
          type: 'rule',
          name: rule.name,
          label: segment,
          rule,
        });
        continue;
      }

      currentPath = currentPath ? `${currentPath}/${segment}` : segment;
      currentFolder = getOrCreateFolder(currentFolder, currentPath, segment);
    }
  }

  return root;
}

export function collectFolderPaths(tree: RuleTreeFolderNode): string[] {
  const paths: string[] = [];

  const walk = (folder: RuleTreeFolderNode) => {
    for (const child of folder.children) {
      if (child.type === 'folder') {
        paths.push(child.path);
        walk(child);
      }
    }
  };

  walk(tree);
  return paths;
}

export function getTopFolderPrefix(ruleName: string): string | null {
  const segments = splitRulePath(ruleName);
  if (segments.length <= 1) return null;
  return segments[0];
}

export function flattenVisibleRuleNames(
  tree: RuleTreeFolderNode,
  expandedFolders: Set<string>
): string[] {
  const visible: string[] = [];

  const walk = (folder: RuleTreeFolderNode) => {
    for (const child of folder.children) {
      if (child.type === 'rule') {
        visible.push(child.name);
        continue;
      }
      if (expandedFolders.has(child.path)) {
        walk(child);
      }
    }
  };

  walk(tree);
  return visible;
}
