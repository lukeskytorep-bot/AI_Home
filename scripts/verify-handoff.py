#!/usr/bin/env python3
"""Compatibility entry point; use the current complete-tree verifier."""
import runpy
from pathlib import Path
runpy.run_path(str(Path(__file__).with_name('verify-source.py')),run_name='__main__')
