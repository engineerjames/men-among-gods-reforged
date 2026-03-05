#!/usr/bin/env python3
"""
Mechanically convert Repository::with_* closure calls to direct self.* access.

This handles the most common patterns:
1. Simple single-expression closures: Repository::with_X(|p| p[i].field) -> self.X[i].field
2. Simple assignment closures: Repository::with_X_mut(|p| p[i].field = val) -> self.X[i].field = val
3. Multi-line closures with tuple returns
4. Nested closures
"""

import re
import sys

def find_matching_paren(text, start):
    """Find the matching closing paren/bracket for the one at start."""
    depth = 0
    i = start
    open_char = text[i]
    close_char = ')' if open_char == '(' else ']' if open_char == '[' else '}'
    while i < len(text):
        if text[i] == open_char:
            depth += 1
        elif text[i] == close_char:
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def find_closure_end(text, start):
    """Find the end of a Repository::with_X(...) call starting at the open paren."""
    return find_matching_paren(text, start)


FIELD_MAP = {
    'with_characters': 'self.characters',
    'with_characters_mut': 'self.characters',
    'with_items': 'self.items',
    'with_items_mut': 'self.items',
    'with_map': 'self.map',
    'with_map_mut': 'self.map',
    'with_globals': 'self.globals',
    'with_globals_mut': 'self.globals',
    'with_effects': 'self.effects',
    'with_effects_mut': 'self.effects',
    'with_see_map': 'self.see_map',
    'with_see_map_mut': 'self.see_map',
    'with_character_templates': 'self.character_templates',
    'with_item_templates': 'self.item_templates',
    'with_ban_list': 'self.ban_list',
    'with_ban_list_mut': 'self.ban_list',
    'with_repo': None,  # Can't auto-convert
    'with_repo_mut': None,
}

GETTER_MAP = {
    'get_last_population_reset_tick': 'self.last_population_reset_tick',
    'set_last_population_reset_tick': 'self.last_population_reset_tick',
    'get_ice_cloak_clock': 'self.ice_cloak_clock',
    'set_ice_cloak_clock': 'self.ice_cloak_clock',
    'get_item_tick_gc_off': 'self.item_tick_gc_off',
    'set_item_tick_gc_off': 'self.item_tick_gc_off',
    'get_item_tick_gc_count': 'self.item_tick_gc_count',
    'set_item_tick_gc_count': 'self.item_tick_gc_count',
    'get_item_tick_expire_counter': 'self.item_tick_expire_counter',
    'set_item_tick_expire_counter': 'self.item_tick_expire_counter',
    'storage_backend': 'self.storage_backend()',
    'latest_message_of_the_day': 'self.latest_message_of_the_day()',
}


def extract_closure_param(closure_text):
    """Extract the closure parameter name from |param| or |param, ...| syntax."""
    m = re.match(r'\|(\w+)\|', closure_text.strip())
    if m:
        return m.group(1)
    return None


def is_simple_expression(body):
    """Check if the closure body is a simple expression (no semicolons except last)."""
    stripped = body.strip()
    # Remove trailing semicolons
    if stripped.endswith(';'):
        stripped = stripped[:-1].strip()
    # Simple: no semicolons, braces only for field access/indexing
    return ';' not in stripped and '\n' not in stripped


def convert_simple_closure(method, closure_text, full_match):
    """Convert a simple closure like Repository::with_X(|p| p[i].field)."""
    field = FIELD_MAP.get(method)
    if field is None:
        return full_match  # Can't convert

    # Extract param and body
    m = re.match(r'\|(\w+)\|\s*(.*)', closure_text.strip(), re.DOTALL)
    if not m:
        return full_match

    param = m.group(1)
    body = m.group(2).strip()

    # Remove wrapping braces if present
    if body.startswith('{') and body.endswith('}'):
        body = body[1:-1].strip()

    if not is_simple_expression(body):
        return None  # Not simple, need multi-line handling

    # Replace param references with self.field
    if 'globals' in method:
        # For globals, param itself IS the globals struct
        result = body.replace(f'{param}.', f'{field}.')
        result = result.replace(f'{param}[', f'{field}[')
    else:
        # For collections, param[i] -> self.field[i]
        result = body.replace(f'{param}[', f'{field}[')
        result = body.replace(f'{param}.', f'{field}.')
        # Also handle bare param references
        result = re.sub(rf'\b{param}\b', field, result)

    return result


def process_file(filepath):
    """Process a single file, converting Repository calls."""
    with open(filepath, 'r') as f:
        content = f.read()

    # Count Repository calls
    count = content.count('Repository::')
    if count == 0:
        print(f"  No Repository:: calls found in {filepath}")
        return

    print(f"  Found {count} Repository:: calls in {filepath}")
    print(f"  Manual conversion needed - script provides analysis only")


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: python convert_repository_calls.py <file>")
        sys.exit(1)

    for f in sys.argv[1:]:
        process_file(f)
