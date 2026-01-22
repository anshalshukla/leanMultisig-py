from __future__ import annotations

from setuptools import setup

setup(
    packages=["lean_multisig_py"],
    package_data={"lean_multisig_py": ["*.so", "*.pyd", "*.pyi", "py.typed"]},
    include_package_data=True,
    zip_safe=False,
)
