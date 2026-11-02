// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_RUSTCONV_H
#define DUCTUS_RUSTCONV_H

#include <QByteArray>
#include <QString>
#include <cstdint>
#include <string>

#include "rust/cxx.h"

/// Strings and bytes across the Rust bridge.
inline QString toQ(const rust::String &s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

inline std::string toStd(const QString &s)
{
    return s.toStdString();
}

inline rust::Slice<const std::uint8_t> bytes(const QByteArray &data)
{
    return {reinterpret_cast<const std::uint8_t *>(data.constData()), static_cast<std::size_t>(data.size())};
}

#endif
