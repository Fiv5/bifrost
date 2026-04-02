#!/usr/bin/env python3
"""Bifrost 项目代码行数统计脚本

统计所有 Git 入库文件的代码行数，按文件类型、语言分类、Rust crate 维度展示。
用法: python3 scripts/count_lines.py
"""

import subprocess
import os
import sys
import collections

BINARY_EXTS = frozenset([
    'png', 'jpg', 'jpeg', 'ico', 'svg', 'gif', 'bmp', 'webp',
    'woff', 'woff2', 'ttf', 'eot',
    'pyc', 'pyo', 'wasm',
    'gz', 'br', 'zip', 'tar', 'bz2', 'xz',
    'lock', 'sum',
    'exe', 'dll', 'so', 'dylib',
    'bin', 'dat', 'db',
])

LANG_GROUPS = [
    ('Rust',       ['rs']),
    ('TypeScript', ['ts', 'tsx', 'mts', 'cts']),
    ('JavaScript', ['js', 'jsx', 'mjs', 'cjs']),
    ('Python',     ['py']),
    ('Go',         ['go']),
    ('Shell',      ['sh', 'ps1']),
    ('Config',     ['json', 'toml', 'yml', 'yaml']),
    ('CSS/Style',  ['css', 'scss', 'less']),
    ('HTML/Web',   ['html', 'astro', 'mdx']),
    ('Docs/Text',  ['md', 'txt']),
    ('Template',   ['tpl']),
]

C_RESET  = '\033[0m'
C_BOLD   = '\033[1m'
C_DIM    = '\033[2m'
C_CYAN   = '\033[36m'
C_GREEN  = '\033[32m'
C_YELLOW = '\033[33m'
C_WHITE  = '\033[37m'
C_BLUE   = '\033[34m'
C_MAGENTA = '\033[35m'

BAR_CHARS = ['█', '▓', '▒', '░']

def supports_color():
    if os.environ.get('NO_COLOR'):
        return False
    return hasattr(sys.stdout, 'isatty') and sys.stdout.isatty()

USE_COLOR = supports_color()

def c(code, text):
    if USE_COLOR:
        return "{}{}{}".format(code, text, C_RESET)
    return str(text)

def fmt_num(n):
    return "{:,}".format(n)

def bar(pct, width=30):
    filled = int(pct / 100 * width)
    if filled < 1 and pct > 0:
        filled = 1
    return '█' * filled + '░' * (width - filled)

def print_header(title):
    width = 72
    print()
    print(c(C_CYAN, '─' * width))
    print(c(C_BOLD + C_CYAN, '  ' + title))
    print(c(C_CYAN, '─' * width))
    print()

def get_git_files():
    root = subprocess.run(
        ['git', 'rev-parse', '--show-toplevel'],
        capture_output=True, text=True
    ).stdout.strip()
    os.chdir(root)
    result = subprocess.run(['git', 'ls-files'], capture_output=True, text=True)
    return [f for f in result.stdout.strip().split('\n') if f]

def count_file(filepath):
    try:
        with open(filepath, 'r', errors='ignore') as fh:
            lines = fh.readlines()
            return len(lines), sum(1 for l in lines if l.strip())
    except Exception:
        return 0, 0

def get_ext(filepath):
    base = os.path.basename(filepath)
    if '.' in base:
        return filepath.rsplit('.', 1)[-1].lower()
    return ''


def main():
    files = get_git_files()

    ext_stats = collections.defaultdict(lambda: {'files': 0, 'lines': 0, 'non_empty': 0})

    for f in files:
        ext = get_ext(f)
        if not ext or ext in BINARY_EXTS:
            continue
        if not os.path.isfile(f):
            continue
        total, non_empty = count_file(f)
        ext_stats[ext]['files'] += 1
        ext_stats[ext]['lines'] += total
        ext_stats[ext]['non_empty'] += non_empty

    sorted_exts = sorted(ext_stats.items(), key=lambda x: x[1]['lines'], reverse=True)
    total_files = sum(v['files'] for _, v in sorted_exts)
    total_lines = sum(v['lines'] for _, v in sorted_exts)
    total_ne = sum(v['non_empty'] for _, v in sorted_exts)

    # ── Section 1: By file type ──
    print_header('按文件类型统计')
    print("  {}  {}  {}  {}  {}".format(
        c(C_BOLD, "{:<10}".format("类型")),
        c(C_BOLD, "{:>8}".format("文件数")),
        c(C_BOLD, "{:>10}".format("总行数")),
        c(C_BOLD, "{:>10}".format("非空行")),
        c(C_BOLD, "{:>8}".format("占比")),
    ))
    print(c(C_DIM, "  " + "─" * 54))

    for ext, stats in sorted_exts:
        pct = (stats['lines'] / total_lines * 100) if total_lines > 0 else 0
        pct_str = "{:.1f}%".format(pct)
        print("  {:<10} {:>8} {:>10} {:>10}  {}".format(
            c(C_GREEN, ".{}".format(ext)),
            fmt_num(stats['files']),
            c(C_YELLOW, fmt_num(stats['lines'])),
            fmt_num(stats['non_empty']),
            c(C_CYAN, "{:>7}".format(pct_str)),
        ))

    print(c(C_DIM, "  " + "─" * 54))
    print("  {:<10} {:>8} {:>10} {:>10}  {:>7}".format(
        c(C_BOLD, "合计"),
        fmt_num(total_files),
        c(C_BOLD + C_YELLOW, fmt_num(total_lines)),
        fmt_num(total_ne),
        "100.0%",
    ))

    # ── Section 2: By language ──
    print_header('按语言分类汇总')

    assigned = set()
    lang_lines = []
    for lang, exts in LANG_GROUPS:
        total = sum(ext_stats[e]['lines'] for e in exts if e in ext_stats)
        for e in exts:
            assigned.add(e)
        if total > 0:
            lang_lines.append((lang, total))

    other_total = sum(v['lines'] for e, v in ext_stats.items() if e not in assigned)
    if other_total > 0:
        lang_lines.append(('Other', other_total))

    lang_lines.sort(key=lambda x: x[1], reverse=True)

    bar_width = 30
    print("  {}  {}  {}  {}".format(
        c(C_BOLD, "{:<14}".format("语言")),
        c(C_BOLD, "{:>10}".format("行数")),
        c(C_BOLD, "{:>8}".format("占比")),
        c(C_BOLD, "分布"),
    ))
    print(c(C_DIM, "  " + "─" * 68))

    colors_cycle = [C_GREEN, C_YELLOW, C_CYAN, C_BLUE, C_MAGENTA, C_WHITE]
    for i, (lang, lines) in enumerate(lang_lines):
        pct = lines / total_lines * 100 if total_lines > 0 else 0
        pct_str = "{:.1f}%".format(pct)
        clr = colors_cycle[i % len(colors_cycle)]
        b = bar(pct, bar_width)
        print("  {:<14} {:>10}  {:>7}  {}".format(
            c(clr, lang),
            c(C_YELLOW, fmt_num(lines)),
            c(C_CYAN, pct_str),
            c(clr, b),
        ))

    print(c(C_DIM, "  " + "─" * 68))
    print("  {:<14} {:>10}  {:>7}".format(
        c(C_BOLD, "合计"),
        c(C_BOLD + C_YELLOW, fmt_num(total_lines)),
        "100.0%",
    ))

    # ── Section 3: Rust crate breakdown ──
    print_header('Rust 代码详情（按 Crate）')

    crate_stats = collections.defaultdict(lambda: {'files': 0, 'lines': 0, 'non_empty': 0})
    for f in files:
        if not f.endswith('.rs') or not os.path.isfile(f):
            continue
        if f.startswith('crates/'):
            parts = f.split('/')
            crate_name = parts[1] if len(parts) >= 2 else 'other'
        elif f.startswith('desktop/'):
            crate_name = 'desktop (tauri)'
        else:
            crate_name = '(root)'
        total_l, non_empty_l = count_file(f)
        crate_stats[crate_name]['files'] += 1
        crate_stats[crate_name]['lines'] += total_l
        crate_stats[crate_name]['non_empty'] += non_empty_l

    sorted_crates = sorted(crate_stats.items(), key=lambda x: x[1]['lines'], reverse=True)
    ct_files = sum(s['files'] for _, s in sorted_crates)
    ct_lines = sum(s['lines'] for _, s in sorted_crates)
    ct_ne = sum(s['non_empty'] for _, s in sorted_crates)

    print("  {}  {}  {}  {}  {}".format(
        c(C_BOLD, "{:<26}".format("Crate")),
        c(C_BOLD, "{:>6}".format("文件")),
        c(C_BOLD, "{:>10}".format("总行数")),
        c(C_BOLD, "{:>10}".format("非空行")),
        c(C_BOLD, "分布"),
    ))
    print(c(C_DIM, "  " + "─" * 68))

    for cname, s in sorted_crates:
        pct = s['lines'] / ct_lines * 100 if ct_lines > 0 else 0
        b = bar(pct, 20)
        print("  {:<26} {:>6} {:>10} {:>10}  {}".format(
            c(C_GREEN, cname),
            s['files'],
            c(C_YELLOW, fmt_num(s['lines'])),
            fmt_num(s['non_empty']),
            c(C_CYAN, b),
        ))

    print(c(C_DIM, "  " + "─" * 68))
    print("  {:<26} {:>6} {:>10} {:>10}".format(
        c(C_BOLD, "合计"),
        ct_files,
        c(C_BOLD + C_YELLOW, fmt_num(ct_lines)),
        fmt_num(ct_ne),
    ))

    # ── Summary ──
    print()
    code_langs = {'rs', 'ts', 'tsx', 'mts', 'cts', 'js', 'jsx', 'mjs', 'cjs', 'py', 'go', 'sh', 'ps1'}
    code_lines = sum(ext_stats[e]['lines'] for e in code_langs if e in ext_stats)
    print(c(C_BOLD, "  📊 总计 Git 入库 {} 个文件，{} 行".format(
        fmt_num(total_files), fmt_num(total_lines)
    )))
    print(c(C_DIM, "     其中纯代码 {} 行（占 {:.1f}%），文档/配置/资源 {} 行".format(
        fmt_num(code_lines),
        code_lines / total_lines * 100 if total_lines > 0 else 0,
        fmt_num(total_lines - code_lines),
    )))
    print()


if __name__ == '__main__':
    main()
