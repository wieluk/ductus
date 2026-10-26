#include "ductus-qt/cpp/textformatter.h"

#include <QTextBlock>
#include <QTextCharFormat>
#include <QTextDocument>
#include <QTextList>

#include <algorithm>

namespace {

// Pixel sizes of small, normal, large and huge text; normal is the box's own 16 px.
constexpr int Sizes[] = {13, 0, 22, 30};

// Characters Markdown would take as formatting, backslash-escaped.
QString escaped(const QString &text, bool blockStart)
{
    QString out;
    out.reserve(text.size());
    for (qsizetype i = 0; i < text.size(); ++i) {
        const QChar c = text.at(i);
        static const QString special = QStringLiteral("\\`*_~<>[]|&");
        bool escape = special.contains(c);
        // At the start of a block these would begin a heading, list or quote.
        if (blockStart && i == 0 && (c == u'#' || c == u'-' || c == u'+' || c == u'>'))
            escape = true;
        // "1." would begin a numbered list.
        if (blockStart && (c == u'.' || c == u')') && i > 0
            && std::all_of(text.begin(), text.begin() + i, [](QChar d) { return d.isDigit(); }))
            escape = true;
        if (escape)
            out += u'\\';
        out += c;
    }
    return out;
}

QString inlineMarkdown(const QTextBlock &block, bool heading)
{
    QString out;
    for (auto it = block.begin(); !it.atEnd(); ++it) {
        const QTextFragment fragment = it.fragment();
        if (!fragment.isValid())
            continue;
        const QTextCharFormat c = fragment.charFormat();
        QString text = fragment.text();
        // Emphasis markers must touch the text, so spaces stay outside them.
        const qsizetype start = std::find_if(text.begin(), text.end(), [](QChar ch) { return !ch.isSpace(); }) - text.begin();
        qsizetype end = text.size();
        while (end > start && text.at(end - 1).isSpace())
            --end;
        const QString lead = text.left(start);
        const QString core = text.mid(start, end - start);
        const QString trail = text.mid(end);
        QString open;
        QString close;
        if (!core.isEmpty()) {
            QStringList style;
            if (c.hasProperty(QTextFormat::ForegroundBrush) && c.foreground().style() != Qt::NoBrush)
                style << QStringLiteral("color:") + c.foreground().color().name();
            if (const int px = c.intProperty(QTextFormat::FontPixelSize); px > 0 && !heading)
                style << QStringLiteral("font-size:%1px").arg(px);
            if (!style.isEmpty()) {
                open += QStringLiteral("<span style=\"%1\">").arg(style.join(u';'));
                close.prepend(QStringLiteral("</span>"));
            }
            if (c.fontUnderline()) {
                open += QStringLiteral("<u>");
                close.prepend(QStringLiteral("</u>"));
            }
            if (!heading && c.fontWeight() >= QFont::DemiBold) {
                open += QStringLiteral("**");
                close.prepend(QStringLiteral("**"));
            }
            if (c.fontItalic()) {
                open += u'*';
                close.prepend(u'*');
            }
            if (c.fontStrikeOut()) {
                open += QStringLiteral("~~");
                close.prepend(QStringLiteral("~~"));
            }
        }
        const bool first = out.isEmpty();
        // Line breaks typed with Shift+Enter stay line breaks.
        const auto lines = [&](const QString &part, bool atStart) {
            QStringList pieces = part.split(QChar::LineSeparator);
            for (qsizetype i = 0; i < pieces.size(); ++i)
                pieces[i] = escaped(pieces[i], atStart && i == 0);
            return pieces.join(QStringLiteral("\\\n"));
        };
        out += lines(lead, first) + open + lines(core, first && lead.isEmpty()) + close + lines(trail, false);
    }
    return out;
}

} // namespace

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

void TextFormatter::changeFormat(const std::function<void(QTextCharFormat &)> &change)
{
    QTextCursor c = cursor();
    if (c.isNull())
        return;
    if (!c.hasSelection())
        c.select(QTextCursor::WordUnderCursor);
    const int from = c.selectionStart();
    const int to = c.selectionEnd();
    c.beginEditBlock();
    // Fragment by fragment: some changes take a property away, which merging can't.
    for (int position = from; position < to;) {
        QTextCursor piece(c.document());
        piece.setPosition(position);
        const QTextBlock block = piece.block();
        int end = to;
        for (auto it = block.begin(); !it.atEnd(); ++it) {
            const QTextFragment f = it.fragment();
            if (f.position() <= position && position < f.position() + f.length()) {
                end = qMin(to, f.position() + f.length());
                break;
            }
        }
        if (end <= position)
            end = qMin(to, block.position() + block.length());
        piece.setPosition(qMax(position, block.position()));
        piece.setPosition(end, QTextCursor::KeepAnchor);
        QTextCharFormat format = piece.charFormat();
        if (piece.hasSelection()) {
            QTextCursor probe(c.document());
            probe.setPosition(piece.selectionStart() + 1);
            format = probe.charFormat();
        }
        change(format);
        piece.setCharFormat(format);
        position = qMax(end, position + 1);
    }
    c.endEditBlock();
    Q_EMIT formatChanged();
}

bool TextFormatter::underline() const
{
    return charFormat().fontUnderline();
}

QString TextFormatter::color() const
{
    const QTextCharFormat f = charFormat();
    return f.hasProperty(QTextFormat::ForegroundBrush) && f.foreground().style() != Qt::NoBrush
               ? f.foreground().color().name()
               : QString();
}

int TextFormatter::size() const
{
    const int px = charFormat().intProperty(QTextFormat::FontPixelSize);
    for (int level = 0; level < 4; ++level)
        if (Sizes[level] == px)
            return level;
    return 1;
}

void TextFormatter::toggleUnderline()
{
    QTextCharFormat f;
    f.setFontUnderline(!underline());
    mergeFormat(f);
}

void TextFormatter::setColor(const QString &color)
{
    changeFormat([&](QTextCharFormat &f) {
        if (color.isEmpty())
            f.clearForeground();
        else
            f.setForeground(QColor(color));
    });
}

void TextFormatter::setSize(int level)
{
    level = qBound(0, level, 3);
    changeFormat([&](QTextCharFormat &f) {
        if (Sizes[level] > 0)
            f.setProperty(QTextFormat::FontPixelSize, Sizes[level]);
        else
            f.clearProperty(QTextFormat::FontPixelSize);
    });
}

QString TextFormatter::markdown(QQuickTextDocument *document) const
{
    if (!document)
        return {};
    const QTextDocument *doc = document->textDocument();
    QString out;
    bool inList = false;
    bool first = true;
    for (QTextBlock block = doc->begin(); block.isValid(); block = block.next()) {
        const QTextList *list = block.textList();
        const int heading = block.blockFormat().headingLevel();
        // Markdown has no empty paragraphs.
        if (!list && heading == 0 && block.text().trimmed().isEmpty())
            continue;
        QString prefix;
        if (list) {
            const int depth = qMax(0, list->format().indent() - 1);
            prefix = QString(depth * 3, u' ')
                     + (list->format().style() == QTextListFormat::ListDecimal
                            ? QString::number(list->itemNumber(block) + 1) + QStringLiteral(". ")
                            : QStringLiteral("- "));
        } else if (heading > 0) {
            prefix = QString(qMin(heading, 6), u'#') + u' ';
        }
        if (!first)
            out += list && inList ? QStringLiteral("\n") : QStringLiteral("\n\n");
        out += prefix + inlineMarkdown(block, heading > 0);
        inList = list != nullptr;
        first = false;
    }
    return out;
}

void TextFormatter::clear(QQuickTextDocument *document)
{
    if (!document)
        return;
    QTextDocument *doc = document->textDocument();
    doc->clear();
    QTextCursor c(doc);
    c.setBlockFormat(QTextBlockFormat());
    c.setCharFormat(QTextCharFormat());
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
