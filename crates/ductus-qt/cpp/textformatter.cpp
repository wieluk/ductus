#include "ductus-qt/cpp/textformatter.h"

#include <QTextBlock>
#include <QTextDocument>
#include <QTextList>

void TextFormatter::setDocument(QQuickTextDocument *document)
{
    if (document == m_document)
        return;
    m_document = document;
    Q_EMIT documentChanged();
    Q_EMIT formatChanged();
}

void TextFormatter::setCursorPosition(int position)
{
    m_cursorPosition = position;
    Q_EMIT formatChanged();
}

QTextCursor TextFormatter::cursor() const
{
    if (!m_document)
        return {};
    QTextCursor cursor(m_document->textDocument());
    if (m_selectionStart != m_selectionEnd) {
        cursor.setPosition(m_selectionStart);
        cursor.setPosition(m_selectionEnd, QTextCursor::KeepAnchor);
    } else {
        cursor.setPosition(m_cursorPosition);
    }
    return cursor;
}

QTextCharFormat TextFormatter::charFormat() const
{
    QTextCursor c = cursor();
    if (c.isNull())
        return {};
    // The format of the first selected character, not the one before it.
    if (c.hasSelection() && c.position() > c.anchor())
        c.setPosition(c.anchor() + 1);
    return c.charFormat();
}

void TextFormatter::mergeFormat(const QTextCharFormat &format)
{
    QTextCursor c = cursor();
    if (c.isNull())
        return;
    if (!c.hasSelection())
        c.select(QTextCursor::WordUnderCursor);
    c.mergeCharFormat(format);
    Q_EMIT formatChanged();
}

bool TextFormatter::bold() const
{
    return charFormat().fontWeight() >= QFont::DemiBold;
}

bool TextFormatter::italic() const
{
    return charFormat().fontItalic();
}

bool TextFormatter::strikeOut() const
{
    return charFormat().fontStrikeOut();
}

int TextFormatter::heading() const
{
    const QTextCursor c = cursor();
    return c.isNull() ? 0 : c.blockFormat().headingLevel();
}

int TextFormatter::list() const
{
    const QTextCursor c = cursor();
    if (c.isNull() || !c.currentList())
        return 0;
    const auto style = c.currentList()->format().style();
    return style == QTextListFormat::ListDecimal ? 2 : 1;
}

void TextFormatter::toggleBold()
{
    QTextCharFormat f;
    f.setFontWeight(bold() ? QFont::Normal : QFont::Bold);
    mergeFormat(f);
}

void TextFormatter::toggleItalic()
{
    QTextCharFormat f;
    f.setFontItalic(!italic());
    mergeFormat(f);
}

void TextFormatter::toggleStrikeOut()
{
    QTextCharFormat f;
    f.setFontStrikeOut(!strikeOut());
    mergeFormat(f);
}

void TextFormatter::setHeading(int level)
{
    QTextCursor c = cursor();
    if (c.isNull())
        return;
    // Heading sizes relative to the 16 px body text, as Markdown viewers show them.
    static constexpr int sizes[] = {0, 3, 2, 1};
    level = qBound(0, level, 3);
    c.beginEditBlock();
    QTextBlockFormat block;
    block.setHeadingLevel(level);
    c.mergeBlockFormat(block);
    QTextCharFormat chars;
    chars.setFontWeight(level > 0 ? QFont::Bold : QFont::Normal);
    chars.setProperty(QTextFormat::FontSizeAdjustment, sizes[level]);
    QTextCursor whole = c;
    whole.setPosition(c.selectionStart());
    whole.movePosition(QTextCursor::StartOfBlock);
    whole.setPosition(c.selectionEnd(), QTextCursor::KeepAnchor);
    whole.movePosition(QTextCursor::EndOfBlock, QTextCursor::KeepAnchor);
    whole.mergeCharFormat(chars);
    c.endEditBlock();
    Q_EMIT formatChanged();
}

bool TextFormatter::autoFormat()
{
    QTextCursor c = cursor();
    if (c.isNull() || c.hasSelection())
        return false;
    const QString typed = c.block().text().left(c.positionInBlock());
    int list = 0;
    int heading = 0;
    if (typed == u"-" || typed == u"*")
        list = 1;
    else if (typed == u"1.")
        list = 2;
    else if (typed == u"#" || typed == u"##" || typed == u"###")
        heading = static_cast<int>(typed.size());
    else
        return false;
    c.beginEditBlock();
    c.movePosition(QTextCursor::StartOfBlock, QTextCursor::KeepAnchor);
    c.removeSelectedText();
    c.endEditBlock();
    m_selectionStart = m_selectionEnd = m_cursorPosition = c.position();
    if (list > 0 && this->list() == 0)
        toggleList(list);
    else if (heading > 0)
        setHeading(heading);
    return true;
}

void TextFormatter::toggleList(int kind)
{
    QTextCursor c = cursor();
    if (c.isNull())
        return;
    c.beginEditBlock();
    if (list() == kind) {
        QTextBlockFormat block = c.blockFormat();
        block.setObjectIndex(-1);
        block.setIndent(0);
        if (QTextList *current = c.currentList()) {
            for (int i = current->count() - 1; i >= 0; --i)
                current->remove(current->item(i));
        }
        c.setBlockFormat(block);
    } else {
        QTextListFormat format;
        format.setStyle(kind == 2 ? QTextListFormat::ListDecimal : QTextListFormat::ListDisc);
        if (QTextList *current = c.currentList())
            current->setFormat(format);
        else
            c.createList(format);
    }
    c.endEditBlock();
    Q_EMIT formatChanged();
}
