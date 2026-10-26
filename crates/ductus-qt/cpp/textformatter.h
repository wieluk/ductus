// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_TEXTFORMATTER_H
#define DUCTUS_TEXTFORMATTER_H

#include <QObject>
#include <QPointer>
#include <QQuickTextDocument>
#include <QTextCursor>
#include <QtQml/qqmlregistration.h>

#include <functional>

/// The rich text parts a text box stores: Markdown, with inline HTML for underline, color and size.
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
    Q_PROPERTY(bool underline READ underline NOTIFY formatChanged)
    /// "" for the text's own color.
    Q_PROPERTY(QString color READ color NOTIFY formatChanged)
    /// 0 small, 1 normal, 2 large, 3 huge.
    Q_PROPERTY(int size READ size NOTIFY formatChanged)
    /// 0 for body text, 1 to 3 for headings.
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
    bool underline() const;
    QString color() const;
    int size() const;
    int heading() const;
    int list() const;

    Q_INVOKABLE void toggleBold();
    Q_INVOKABLE void toggleItalic();
    Q_INVOKABLE void toggleStrikeOut();
    Q_INVOKABLE void toggleUnderline();
    /// "" goes back to the text's own color.
    Q_INVOKABLE void setColor(const QString &color);
    Q_INVOKABLE void setSize(int level);
    /// What a text box stores: the formats above that Markdown has, and inline HTML for the rest.
    Q_INVOKABLE QString markdown(QQuickTextDocument *document) const;
    /// Empties a document, formats included, so a new box starts as plain text.
    Q_INVOKABLE void clear(QQuickTextDocument *document);
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
    /// Changes every character format in the selection (or the word under the cursor).
    void changeFormat(const std::function<void(QTextCharFormat &)> &change);

    QPointer<QQuickTextDocument> m_document;
    int m_selectionStart = 0;
    int m_selectionEnd = 0;
    int m_cursorPosition = 0;
};

#endif
