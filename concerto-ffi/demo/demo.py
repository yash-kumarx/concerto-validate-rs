#!/usr/bin/env python3

# tiny ctypes demo for concerto-ffi
#
# the annoying ctypes footgun here is restype. if you forget to set it,
# ctypes assumes c_int and happily mangles your returned pointer on 64-bit
# systems. found that out the fun way

import ctypes
import json
import platform
from pathlib import Path


def find_library_path() -> Path:
    repo_root = Path(__file__).resolve().parents[2]
    release_dir = repo_root / "target" / "release"

    system = platform.system()
    if system == "Darwin":
        lib_name = "libconcerto_ffi.dylib"
    elif system == "Linux":
        lib_name = "libconcerto_ffi.so"
    elif system == "Windows":
        lib_name = "concerto_ffi.dll"
    else:
        raise RuntimeError(f"unsupported platform: {system}")

    lib_path = release_dir / lib_name
    if not lib_path.exists():
        raise FileNotFoundError(
            f"can't find {lib_name} in {release_dir}\n"
            "run: cargo build --release -p concerto-ffi"
        )

    return lib_path


LIB = ctypes.CDLL(str(find_library_path()))

LIB.concerto_model_manager_new.argtypes = []
LIB.concerto_model_manager_new.restype = ctypes.c_void_p

LIB.concerto_model_manager_free.argtypes = [ctypes.c_void_p]
LIB.concerto_model_manager_free.restype = None

LIB.concerto_add_model.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
LIB.concerto_add_model.restype = ctypes.c_void_p

LIB.concerto_validate_instance.argtypes = [
    ctypes.c_void_p,
    ctypes.c_char_p,
    ctypes.c_char_p,
]
LIB.concerto_validate_instance.restype = ctypes.c_void_p

LIB.concerto_free_string.argtypes = [ctypes.c_char_p]
LIB.concerto_free_string.restype = None


def decode_owned_string(raw_ptr) -> str:
    if not raw_ptr:
        return ""

    raw_bytes = ctypes.cast(raw_ptr, ctypes.c_char_p).value
    text = raw_bytes.decode("utf-8") if raw_bytes else ""
    LIB.concerto_free_string(ctypes.cast(raw_ptr, ctypes.c_char_p))
    return text


def validate(mm, instance_json: str, type_name: str) -> dict:
    raw_ptr = LIB.concerto_validate_instance(
        mm,
        instance_json.encode("utf-8"),
        type_name.encode("utf-8"),
    )
    return json.loads(decode_owned_string(raw_ptr))


PERSON_MODEL_JSON = json.dumps(
    {
        "$class": "concerto.metamodel@1.0.0.Model",
        "namespace": "org.example@1.0.0",
        "declarations": [
            {
                "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
                "name": "Person",
                "isAbstract": False,
                "properties": [
                    {
                        "$class": "concerto.metamodel@1.0.0.StringProperty",
                        "name": "email",
                        "isArray": False,
                        "isOptional": False,
                    },
                    {
                        "$class": "concerto.metamodel@1.0.0.StringProperty",
                        "name": "firstName",
                        "isArray": False,
                        "isOptional": False,
                    },
                    {
                        "$class": "concerto.metamodel@1.0.0.IntegerProperty",
                        "name": "age",
                        "isArray": False,
                        "isOptional": False,
                        "validator": {
                            "$class": "concerto.metamodel@1.0.0.IntegerDomainValidator",
                            "lower": 0,
                            "upper": 150,
                        },
                    },
                ],
            }
        ],
    }
)

VALID_INSTANCE_JSON = json.dumps(
    {
        "$class": "org.example@1.0.0.Person",
        "email": "alice@example.com",
        "firstName": "Alice",
        "age": 30,
    }
)

INVALID_INSTANCE_JSON = json.dumps(
    {
        "$class": "org.example@1.0.0.Person",
        "email": "alice@example.com",
        "firstName": "Alice",
        "age": 999,
        "extraField": "should not be here",
    }
)


def main() -> None:
    mm = LIB.concerto_model_manager_new()
    if not mm:
        raise RuntimeError("couldn't allocate model manager")

    try:
        load_err_ptr = LIB.concerto_add_model(mm, PERSON_MODEL_JSON.encode("utf-8"))
        if load_err_ptr:
            raise RuntimeError(decode_owned_string(load_err_ptr))

        print("loaded model\n")

        print("valid instance")
        print(json.dumps(validate(mm, VALID_INSTANCE_JSON, "org.example@1.0.0.Person"), indent=2))
        print()

        print("invalid instance")
        print(
            json.dumps(
                validate(mm, INVALID_INSTANCE_JSON, "org.example@1.0.0.Person"),
                indent=2,
            )
        )
    finally:
        LIB.concerto_model_manager_free(mm)


if __name__ == "__main__":
    main()
