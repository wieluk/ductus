// Include guards, not #pragma once: the build reaches headers through two include paths.
#ifndef DUCTUS_TEXTFORMATTER_H
#define DUCTUS_TEXTFORMATTER_H

#include <QObject>
#include <QPointer>
#include <QQuickTextDocument>
#include <QTextCursor>
#include <QtQml/qqmlregistration.h>

/// The rich text parts Markdown can store.
class TextFormatter : public QObject
{
    Q_OBJECT
    QML_ELEMENT

    Q_PROPERTY(QQuickTextDocument *document READ document WRITE setDocument NOTIFY documentChanged)
    Q_PROPERTY(int selectionStart MEMBER m_selectionStart NOTIFY formatChanged)
    Q_PROPERTY(int selectionEnd MEMBER m_selectionEnd NOTIFY formatChanged)
    Q_PROPERTY(int cursorPosition MEMBER m_cursorPosition WRITE setCursorPosition NOTIFY formatChanged)
    Q_PROPERTY(bool bold READ bold NOTIFY formatChanged)
    Q_PROPERTY(bool italic READ italic NOTIFY formatChanged)
    Q_PROPERTY(bool strikeOut READ strikeOut NOTIFY formatChanged)
    /// 0 for body text, 1–3 for headings.
    Q_PROPERTY(int heading READ heading NOTIFY formatChanged)
    /// 0 none, 1 bullets, 2 numbers.
    Q_PROPERTY(int list READ list NOTIFY formatChanged)

public:
    using QObject::QObject;

    QQuickTextDocument *document() const { return m_document; }
    void setDocument(QQuickTextDocument *document);
    void setCursorPosition(int position);
    bool bold() const;
    bool italic() const;
    bool strikeOut() const;
    int heading() const;
    int list() const;

    Q_INVOKABLE void toggleBold();
    Q_INVOKABLE void toggleItalic();
    Q_INVOKABLE void toggleStrikeOut();
    Q_INVOKABLE void setHeading(int level);
    Q_INVOKABLE void toggleList(int kind);
    /// Returns true when it formatted, so the space isn't typed.
    Q_INVOKABLE bool autoFormat();

Q_SIGNALS:
    void documentChanged();
    void formatChanged();

private:
    QTextCursor cursor() const;
    QTextCharFormat charFormat() const;
    void mergeFormat(const QTextCharFormat &format);

    QPointer<QQuickTextDocument> m_document;
    int m_selectionStart = 0;
    int m_selectionEnd = 0;
    int m_cursorPosition = 0;
};

#endif
