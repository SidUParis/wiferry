# Build with: pyinstaller --clean --noconfirm wiferry.spec

from pathlib import Path


root = Path(SPECPATH)

analysis = Analysis(
    [str(root / "launcher.py")],
    pathex=[str(root)],
    binaries=[],
    datas=[(str(root / "wiferry" / "static"), "wiferry/static")],
    hiddenimports=["multipart", "PIL._tkinter_finder"],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=1,
)
archive = PYZ(analysis.pure)

executable = EXE(
    archive,
    analysis.scripts,
    analysis.binaries,
    analysis.datas,
    [],
    name="wiferry",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,
)
