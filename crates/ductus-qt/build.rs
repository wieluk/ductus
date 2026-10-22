use cxx_qt_build::{CxxQtBuilder, QResources, QmlFile, QmlModule};

fn main() {
    let qml = QmlModule::new("Ductus").qml_files([
        QmlFile::from("qml/Main.qml"),
        QmlFile::from("qml/Theme.qml").singleton(true),
        QmlFile::from("qml/TitleBar.qml"),
        QmlFile::from("qml/Ribbon.qml"),
        QmlFile::from("qml/RibbonButton.qml"),
        QmlFile::from("qml/ColorSwatch.qml"),
        QmlFile::from("qml/ResizeHandles.qml"),
        QmlFile::from("qml/AppMenu.qml"),
        QmlFile::from("qml/AppMenuItem.qml"),
        QmlFile::from("qml/AppMenuSeparator.qml"),
        QmlFile::from("qml/NotebookTree.qml"),
        QmlFile::from("qml/Toast.qml"),
        QmlFile::from("qml/AppDialog.qml"),
        QmlFile::from("qml/SettingsDialog.qml"),
        QmlFile::from("qml/PenBar.qml"),
        QmlFile::from("qml/SelectionBar.qml"),
        QmlFile::from("qml/TextLayer.qml"),
        QmlFile::from("qml/MathLayer.qml"),
        QmlFile::from("qml/MathSnippets.qml").singleton(true),
        QmlFile::from("qml/LinePatternIcon.qml"),
        QmlFile::from("qml/SearchBox.qml"),
        QmlFile::from("qml/UpdateDialog.qml"),
    ]);
    let mut builder = CxxQtBuilder::new_qml_module(qml)
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("Svg")
        .qt_module("Test")
        .qt_module("Widgets")
        .files(["src/ffi.rs"])
        .cpp_files([
            "cpp/app.cpp",
            "cpp/inkcanvas.h",
            "cpp/inkcanvas.cpp",
            "cpp/appcontroller.h",
            "cpp/appcontroller.cpp",
            "cpp/uitest.cpp",
            "cpp/textformatter.h",
            "cpp/textformatter.cpp",
            "cpp/updater.h",
            "cpp/updater.cpp",
        ])
        .qrc_resources(QResources::from(icons()));
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    // Screenshots go through the desktop portal on D-Bus.
    if target_os == "linux" {
        builder = builder.qt_module("DBus");
    }
    // Android has no printing.
    if target_os != "android" {
        builder = builder.qt_module("PrintSupport");
    } else {
        link_android_qt_libraries();
    }
    let version = format!("\"{}\"", env!("CARGO_PKG_VERSION"));
    // The UI test driver needs Qt's private QPA headers; cxx-qt marks raw cc access unsafe.
    #[allow(unsafe_code)]
    let builder = unsafe {
        builder.cc_builder(|cc| {
            for dir in qt_private_includes() {
                cc.include(dir);
            }
            cc.define("DUCTUS_VERSION", version.as_str());
            if target_os == "android" {
                cc.define("DUCTUS_NO_PRINTING", None);
            }
            // Development builds don't look for updates.
            if std::env::var("PROFILE").as_deref() == Ok("debug") {
                cc.define("DUCTUS_DEVELOPMENT_BUILD", None);
            }
        })
    };
    builder.build();
    #[cfg(windows)]
    windows_icon();
}

/// Qt takes the window icon from IDI_ICON1.
#[cfg(windows)]
fn windows_icon() {
    let ico = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/icons/app.ductus.Ductus.ico")
        .canonicalize()
        .expect("data/icons/app.ductus.Ductus.ico exists");
    println!("cargo::rerun-if-changed={}", ico.display());
    let ico = ico.display().to_string();
    let ico = ico.strip_prefix(r"\\?\").unwrap_or(&ico).replace('\\', "/");
    let rc = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ductus.rc");
    std::fs::write(&rc, format!("IDI_ICON1 ICON \"{ico}\"\n")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}

fn icons() -> Vec<String> {
    println!("cargo::rerun-if-changed=qml/icons");
    let mut icons: Vec<String> = std::fs::read_dir("qml/icons")
        .expect("qml/icons exists")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.ends_with(".svg").then(|| format!("qml/icons/{name}"))
        })
        .collect();
    icons.sort();
    icons
}

fn qmake_query(var: &str) -> Option<String> {
    let qmake = std::env::var("QMAKE").ok();
    let candidates = qmake.iter().map(String::as_str).chain(["qmake6", "qmake"]);
    for qmake in candidates {
        let Ok(out) = std::process::Command::new(qmake)
            .args(["-query", var])
            .output()
        else {
            continue;
        };
        if out.status.success() {
            return Some(String::from_utf8_lossy(&out.stdout).trim().to_owned());
        }
    }
    None
}

fn qt_private_includes() -> Vec<std::path::PathBuf> {
    let (Some(headers), Some(libs), Some(version)) = (
        qmake_query("QT_INSTALL_HEADERS"),
        qmake_query("QT_INSTALL_LIBS"),
        qmake_query("QT_VERSION"),
    ) else {
        panic!("qmake not found; set QMAKE to the Qt 6 qmake");
    };
    let mut dirs = Vec::new();
    for module in ["QtCore", "QtGui"] {
        for base in [
            std::path::Path::new(&headers).join(module).join(&version),
            std::path::Path::new(&libs)
                .join(format!("{module}.framework"))
                .join("Headers")
                .join(&version),
        ] {
            dirs.push(base.join(module));
            dirs.push(base);
        }
    }
    dirs.retain(|dir| dir.is_dir());
    assert!(!dirs.is_empty(), "Qt's private headers were not found");
    dirs
}

/// Qt for Android suffixes its libraries with the ABI; cxx-qt links the plain names.
fn link_android_qt_libraries() {
    let abi = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64-v8a",
        Ok("arm") => "armeabi-v7a",
        Ok("x86_64") => "x86_64",
        Ok("x86") => "x86",
        other => panic!("unsupported Android architecture {other:?}"),
    };
    let libs = qmake_query("QT_INSTALL_LIBS").expect("qmake not found; set QMAKE");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("qt-libs");
    std::fs::create_dir_all(&out).unwrap();
    let suffix = format!("_{abi}.so");
    for entry in std::fs::read_dir(&libs)
        .expect("Qt's lib folder exists")
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(stem) = name.strip_suffix(&suffix) {
            let link = out.join(format!("{stem}.so"));
            let _ = std::fs::remove_file(&link);
            #[cfg(unix)]
            std::os::unix::fs::symlink(entry.path(), &link).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(entry.path(), &link).unwrap();
        }
    }
    println!("cargo::rustc-link-search={}", out.display());
}
