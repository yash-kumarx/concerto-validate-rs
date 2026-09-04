#!/usr/bin/env python3
"""
complex_demo.py  —  concerto-ffi Python demo

Run after:
    cargo build --release -p concerto-ffi   (from workspace root)
    python3 concerto-ffi/demo/complex_demo.py
"""

import ctypes
import json
import platform
from pathlib import Path


def _load_lib() -> ctypes.CDLL:
    repo_root = Path(__file__).resolve().parents[2]
    release_dir = repo_root / "target" / "release"
    name = {
        "Darwin": "libconcerto_ffi.dylib",
        "Linux": "libconcerto_ffi.so",
        "Windows": "concerto_ffi.dll",
    }.get(platform.system())
    lib_path = release_dir / name
    if not lib_path.exists():
        raise FileNotFoundError(f"{lib_path} not found\nrun: cargo build --release -p concerto-ffi")
    lib = ctypes.CDLL(str(lib_path))
    lib.concerto_model_manager_new.argtypes = []
    lib.concerto_model_manager_new.restype = ctypes.c_void_p
    lib.concerto_model_manager_free.argtypes = [ctypes.c_void_p]
    lib.concerto_model_manager_free.restype = None
    lib.concerto_add_model.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
    lib.concerto_add_model.restype = ctypes.c_void_p
    lib.concerto_validate_instance.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
    lib.concerto_validate_instance.restype = ctypes.c_void_p
    lib.concerto_free_string.argtypes = [ctypes.c_char_p]
    lib.concerto_free_string.restype = None
    return lib


LIB = _load_lib()


def _consume_ptr(raw_ptr) -> str:
    if not raw_ptr:
        return ""
    raw_bytes = ctypes.cast(raw_ptr, ctypes.c_char_p).value
    text = raw_bytes.decode("utf-8") if raw_bytes else ""
    LIB.concerto_free_string(ctypes.cast(raw_ptr, ctypes.c_char_p))
    return text


def add_model(mm, model_dict: dict) -> None:
    err_ptr = LIB.concerto_add_model(mm, json.dumps(model_dict).encode())
    if err_ptr:
        raise RuntimeError(_consume_ptr(err_ptr))


def validate(mm, instance_dict: dict, type_name: str) -> dict:
    raw = LIB.concerto_validate_instance(
        mm,
        json.dumps(instance_dict).encode(),
        type_name.encode(),
    )
    return json.loads(_consume_ptr(raw))


# ── model ──────────────────────────────────────────────────────────────────

MODEL = {
    "$class": "concerto.metamodel@1.0.0.Model",
    "namespace": "org.hr@1.0.0",
    "declarations": [
        {
            "$class": "concerto.metamodel@1.0.0.EnumDeclaration",
            "name": "Role",
            "properties": [
                {"$class": "concerto.metamodel@1.0.0.EnumProperty", "name": "ADMIN"},
                {"$class": "concerto.metamodel@1.0.0.EnumProperty", "name": "STAFF"},
            ],
        },
        {
            "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
            "name": "Employee",
            "isAbstract": False,
            "properties": [
                {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "name", "isArray": False, "isOptional": False},
                {
                    "$class": "concerto.metamodel@1.0.0.IntegerProperty",
                    "name": "age",
                    "isArray": False,
                    "isOptional": False,
                    "validator": {
                        "$class": "concerto.metamodel@1.0.0.IntegerDomainValidator",
                        "lower": 18,
                        "upper": 65,
                    },
                },
                {
                    "$class": "concerto.metamodel@1.0.0.ObjectProperty",
                    "name": "role",
                    "isArray": False,
                    "isOptional": False,
                    "type": {"$class": "concerto.metamodel@1.0.0.TypeIdentifier", "name": "Role"},
                },
            ],
        },
    ],
}

VALID = {
    "$class": "org.hr@1.0.0.Employee",
    "name": "Priya",
    "age": 30,
    "role": "ADMIN",
}

# 3 errors: age > 65, role not in enum, ghost is unknown field
INVALID = {
    "$class": "org.hr@1.0.0.Employee",
    "name": "Priya",
    "age": 200,
    "role": "CEO",
    "ghost": "extra",
}


def print_result(label: str, result: dict) -> None:
    print(f"\n── {label} ──")
    if result.get("valid"):
        print("  ✓  VALID")
    else:
        errors = result.get("errors", [])
        print(f"  ✗  INVALID  ({len(errors)} errors)")
        for e in errors:
            print(f"     [{e.get('path', '?')}]  {e.get('message', '?')}")


def main() -> None:
    mm = LIB.concerto_model_manager_new()
    if not mm:
        raise RuntimeError("model manager allocation failed")
    try:
        add_model(mm, MODEL)
        print("model loaded  (org.hr@1.0.0 — Role enum, Employee)")

        print_result("valid instance", validate(mm, VALID, "org.hr@1.0.0.Employee"))
        print_result("invalid instance (3 errors)", validate(mm, INVALID, "org.hr@1.0.0.Employee"))
    finally:
        LIB.concerto_model_manager_free(mm)


if __name__ == "__main__":
    main()
