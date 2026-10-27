#!/usr/bin/env python3
"""Generates Supercy.xcodeproj from the source tree.

The project file is written deterministically (object IDs are hashes of their names), so running
this script again gives the same file and diffs stay readable. Run it after adding or removing a
Swift file:

    python3 apps/macos/tools/generate_project.py

Two targets:
  * Supercy (app): SupercyKit + SupercyApp sources + the generated bindings.
  * SupercyKitTests (unit tests, no host app): SupercyKit sources + bindings + tests. The tests
    never compile the app's system-facing code (real pasteboard, keychain, key events).
"""

import hashlib
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
APP_DIR = HERE.parent  # apps/macos
PROJECT = APP_DIR / "Supercy.xcodeproj"
BINDINGS = "../../bindings/swift/SupercyCore.swift"
RUST_LIB = "$(PROJECT_DIR)/../../target/aarch64-apple-darwin/release/libcy_ffi.a"
MODULE_MAP = "$(PROJECT_DIR)/../../bindings/swift/SupercyCoreFFI.modulemap"
BUNDLE_ID = "io.github.weikxs.Supercy"
VERSION = "0.1.0"


def oid(*parts: str) -> str:
    return hashlib.md5("/".join(parts).encode()).hexdigest()[:24].upper()


def swift_files(folder: str) -> list[str]:
    return sorted(str(p.relative_to(APP_DIR)) for p in (APP_DIR / folder).glob("*.swift"))


def quote(value: str) -> str:
    safe = value != "" and all(c.isascii() and (c.isalnum() or c in "_./") for c in value)
    return value if safe else '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render(value, indent: int) -> str:
    pad = "\t" * indent
    if isinstance(value, dict):
        inner = "".join(f"{pad}\t{quote(k)} = {render(v, indent + 1)};\n" for k, v in value.items())
        return "{\n" + inner + pad + "}"
    if isinstance(value, list):
        inner = "".join(f"{pad}\t{render(v, indent + 1)},\n" for v in value)
        return "(\n" + inner + pad + ")"
    return quote(str(value))


KIT = swift_files("Sources/SupercyKit")
APP = swift_files("Sources/SupercyApp")
TESTS = swift_files("Tests/SupercyKitTests")

objects: dict[str, dict] = {}


def file_ref(path: str) -> str:
    key = oid("ref", path)
    objects[key] = {"isa": "PBXFileReference", "lastKnownFileType": "sourcecode.swift",
                    "name": pathlib.Path(path).name, "path": path, "sourceTree": "SOURCE_ROOT"}
    return key


refs = {path: file_ref(path) for path in KIT + APP + TESTS + [BINDINGS]}


def group(name: str, children: list[str]) -> str:
    key = oid("group", name)
    objects[key] = {"isa": "PBXGroup", "children": children, "name": name, "sourceTree": "<group>"}
    return key


def product(name: str, kind: str) -> str:
    key = oid("product", name)
    objects[key] = {"isa": "PBXFileReference", "explicitFileType": kind, "includeInIndex": "0",
                    "path": name, "sourceTree": "BUILT_PRODUCTS_DIR"}
    return key


APP_PRODUCT = product("Supercy.app", "wrapper.application")
TEST_PRODUCT = product("SupercyKitTests.xctest", "wrapper.cfbundle")
PRODUCTS = group("Products", [APP_PRODUCT, TEST_PRODUCT])
MAIN = group("Supercy", [
    group("SupercyKit", [refs[p] for p in KIT]),
    group("SupercyApp", [refs[p] for p in APP]),
    group("Bindings", [refs[BINDINGS]]),
    group("SupercyKitTests", [refs[p] for p in TESTS]),
    PRODUCTS,
])


def phase(target: str, isa: str, files: list[str]) -> str:
    key = oid("phase", target, isa)
    build_files = []
    for path in files:
        build = oid("build", target, path)
        objects[build] = {"isa": "PBXBuildFile", "fileRef": refs[path]}
        build_files.append(build)
    objects[key] = {"isa": isa, "buildActionMask": "2147483647", "files": build_files,
                    "runOnlyForDeploymentPostprocessing": "0"}
    return key


def config_list(owner: str, debug: dict, release: dict) -> str:
    ids = []
    for name, settings in (("Debug", debug), ("Release", release)):
        key = oid("config", owner, name)
        objects[key] = {"isa": "XCBuildConfiguration", "buildSettings": settings, "name": name}
        ids.append(key)
    key = oid("configlist", owner)
    objects[key] = {"isa": "XCConfigurationList", "buildConfigurations": ids,
                    "defaultConfigurationIsVisible": "0", "defaultConfigurationName": "Release"}
    return key


COMMON = {
    "SDKROOT": "macosx",
    "MACOSX_DEPLOYMENT_TARGET": "14.0",
    "ARCHS": "arm64",
    "SWIFT_VERSION": "5.0",
    "SWIFT_STRICT_CONCURRENCY": "minimal",
    "SWIFT_TREAT_WARNINGS_AS_ERRORS": "YES",
    "GCC_TREAT_WARNINGS_AS_ERRORS": "YES",
    "CLANG_ENABLE_MODULES": "YES",
    "CODE_SIGN_STYLE": "Manual",
    "CODE_SIGN_IDENTITY": "-",
    "DEVELOPMENT_TEAM": "",
    "ENABLE_USER_SCRIPT_SANDBOXING": "YES",
    "OTHER_SWIFT_FLAGS": f"-Xcc -fmodule-map-file={MODULE_MAP}",
    "HEADER_SEARCH_PATHS": "$(PROJECT_DIR)/../../bindings/swift",
    "OTHER_LDFLAGS": RUST_LIB,
    "MARKETING_VERSION": VERSION,
    "CURRENT_PROJECT_VERSION": "1",
}
DEBUG = {**COMMON, "ONLY_ACTIVE_ARCH": "YES", "SWIFT_OPTIMIZATION_LEVEL": "-Onone", "ENABLE_TESTABILITY": "YES",
         "SWIFT_ACTIVE_COMPILATION_CONDITIONS": "DEBUG", "DEBUG_INFORMATION_FORMAT": "dwarf"}
RELEASE = {**COMMON, "SWIFT_OPTIMIZATION_LEVEL": "-O", "SWIFT_COMPILATION_MODE": "wholemodule",
           "DEBUG_INFORMATION_FORMAT": "dwarf-with-dsym"}

APP_SETTINGS = {
    "PRODUCT_NAME": "Supercy",
    "PRODUCT_BUNDLE_IDENTIFIER": BUNDLE_ID,
    "GENERATE_INFOPLIST_FILE": "YES",
    "INFOPLIST_KEY_LSUIElement": "YES",
    "INFOPLIST_KEY_CFBundleDisplayName": "Supercy",
    "INFOPLIST_KEY_LSApplicationCategoryType": "public.app-category.productivity",
    "INFOPLIST_KEY_NSHumanReadableCopyright": "",
    "ENABLE_HARDENED_RUNTIME": "YES",
    "LD_RUNPATH_SEARCH_PATHS": "@executable_path/../Frameworks",
}
TEST_SETTINGS = {
    "PRODUCT_NAME": "SupercyKitTests",
    "PRODUCT_BUNDLE_IDENTIFIER": BUNDLE_ID + "Tests",
    "GENERATE_INFOPLIST_FILE": "YES",
}


def target(name: str, product_ref: str, kind: str, sources: list[str], settings: dict) -> str:
    key = oid("target", name)
    objects[key] = {
        "isa": "PBXNativeTarget",
        "buildConfigurationList": config_list(name, {**DEBUG, **settings}, {**RELEASE, **settings}),
        "buildPhases": [phase(name, "PBXSourcesBuildPhase", sources), phase(name, "PBXFrameworksBuildPhase", [])],
        "buildRules": [],
        "dependencies": [],
        "name": name,
        "productName": name,
        "productReference": product_ref,
        "productType": kind,
    }
    return key


APP_TARGET = target("Supercy", APP_PRODUCT, "com.apple.product-type.application", KIT + APP + [BINDINGS], APP_SETTINGS)
TEST_TARGET = target("SupercyKitTests", TEST_PRODUCT, "com.apple.product-type.bundle.unit-test",
                     KIT + TESTS + [BINDINGS], TEST_SETTINGS)

ROOT = oid("project", "Supercy")
objects[ROOT] = {
    "isa": "PBXProject",
    "attributes": {"BuildIndependentTargetsInParallel": "1", "LastUpgradeCheck": "1500"},
    "buildConfigurationList": config_list("project", DEBUG, RELEASE),
    "compatibilityVersion": "Xcode 14.0",
    "developmentRegion": "en",
    "hasScannedForEncodings": "0",
    "knownRegions": ["en", "zh-Hans", "Base"],
    "mainGroup": MAIN,
    "productRefGroup": PRODUCTS,
    "projectDirPath": "",
    "projectRoot": "",
    "targets": [APP_TARGET, TEST_TARGET],
}


def write_project() -> None:
    lines = ["// !$*UTF8*$!", "{", "\tarchiveVersion = 1;", "\tclasses = {", "\t};", "\tobjectVersion = 56;", "\tobjects = {"]
    for key in sorted(objects):
        lines.append(f"\t\t{key} = {render(objects[key], 2)};")
    lines += ["\t};", f"\trootObject = {ROOT};", "}", ""]
    PROJECT.mkdir(exist_ok=True)
    (PROJECT / "project.pbxproj").write_text("\n".join(lines))


SCHEME = """<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion = "1500" version = "1.7">
   <BuildAction parallelizeBuildables = "YES" buildImplicitDependencies = "YES">
      <BuildActionEntries>
         <BuildActionEntry buildForTesting = "YES" buildForRunning = "YES" buildForProfiling = "YES" buildForArchiving = "YES" buildForAnalyzing = "YES">
            <BuildableReference BuildableIdentifier = "primary" BlueprintIdentifier = "{app}" BuildableName = "Supercy.app" BlueprintName = "Supercy" ReferencedContainer = "container:Supercy.xcodeproj">
            </BuildableReference>
         </BuildActionEntry>
      </BuildActionEntries>
   </BuildAction>
   <TestAction buildConfiguration = "Release" selectedDebuggerIdentifier = "" selectedLauncherIdentifier = "Xcode.IDEFoundation.Launcher.PosixSpawn" shouldUseLaunchSchemeArgsEnv = "YES">
      <Testables>
         <TestableReference skipped = "NO" parallelizable = "NO">
            <BuildableReference BuildableIdentifier = "primary" BlueprintIdentifier = "{tests}" BuildableName = "SupercyKitTests.xctest" BlueprintName = "SupercyKitTests" ReferencedContainer = "container:Supercy.xcodeproj">
            </BuildableReference>
         </TestableReference>
      </Testables>
   </TestAction>
   <LaunchAction buildConfiguration = "Debug" selectedDebuggerIdentifier = "Xcode.DebuggerFoundation.Debugger.LLDB" selectedLauncherIdentifier = "Xcode.DebuggerFoundation.Launcher.LLDB" launchStyle = "0" useCustomWorkingDirectory = "NO" ignoresPersistentStateOnLaunch = "NO" debugDocumentVersioning = "YES" allowLocationSimulation = "YES">
      <BuildableProductRunnable runnableDebuggingMode = "0">
         <BuildableReference BuildableIdentifier = "primary" BlueprintIdentifier = "{app}" BuildableName = "Supercy.app" BlueprintName = "Supercy" ReferencedContainer = "container:Supercy.xcodeproj">
         </BuildableReference>
      </BuildableProductRunnable>
   </LaunchAction>
   <ArchiveAction buildConfiguration = "Release" revealArchiveInOrganizer = "YES">
   </ArchiveAction>
</Scheme>
"""


def write_scheme() -> None:
    folder = PROJECT / "xcshareddata" / "xcschemes"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "Supercy.xcscheme").write_text(SCHEME.replace("{app}", APP_TARGET).replace("{tests}", TEST_TARGET))


if __name__ == "__main__":
    write_project()
    write_scheme()
    print(f"wrote {PROJECT.relative_to(APP_DIR.parent.parent)} ({len(KIT)} kit, {len(APP)} app, {len(TESTS)} test files)")
