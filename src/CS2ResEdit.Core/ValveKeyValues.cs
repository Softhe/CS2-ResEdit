using System.Text;

namespace Softhe.CS2ResEdit.Core;

public sealed record KeyValueEntry(string Name, string? Value, KeyValuesObject? Object);

public sealed class KeyValuesObject(IReadOnlyList<KeyValueEntry> entries)
{
    public IReadOnlyList<KeyValueEntry> Entries { get; } = entries;
    public string? GetString(string name) => Entries.LastOrDefault(x =>
        x.Object is null && string.Equals(x.Name, name, StringComparison.OrdinalIgnoreCase))?.Value;
    public IEnumerable<KeyValuesObject> GetObjects(string name) => Entries.Where(x =>
        x.Object is not null && string.Equals(x.Name, name, StringComparison.OrdinalIgnoreCase)).Select(x => x.Object!);
}

public static class ValveKeyValues
{
    public static KeyValuesObject Parse(string text)
    {
        var parser = new Parser(text);
        var result = parser.ReadObject(false);
        if (parser.ReadToken() is not null) throw new InvalidDataException("Unexpected trailing KeyValues content.");
        return result;
    }

    private sealed class Parser(string text)
    {
        private int position;

        public KeyValuesObject ReadObject(bool requiresClose)
        {
            var entries = new List<KeyValueEntry>();
            while (true)
            {
                var key = ReadToken();
                if (key is null)
                {
                    if (requiresClose) throw new InvalidDataException("KeyValues object is not closed.");
                    return new KeyValuesObject(entries);
                }
                if (key.Kind == TokenKind.Close)
                {
                    if (!requiresClose) throw new InvalidDataException("Unexpected closing brace.");
                    return new KeyValuesObject(entries);
                }
                if (key.Kind != TokenKind.Value) throw new InvalidDataException("A KeyValues key was expected.");

                var value = ReadToken() ?? throw new InvalidDataException($"KeyValues entry '{key.Value}' has no value.");
                if (value.Kind == TokenKind.Open)
                    entries.Add(new KeyValueEntry(key.Value, null, ReadObject(true)));
                else if (value.Kind == TokenKind.Value)
                    entries.Add(new KeyValueEntry(key.Value, value.Value, null));
                else throw new InvalidDataException($"KeyValues entry '{key.Value}' has an invalid value.");
            }
        }

        public Token? ReadToken()
        {
            SkipTrivia();
            if (position >= text.Length) return null;
            if (text[position] == '{') { position++; return new Token(TokenKind.Open, "{"); }
            if (text[position] == '}') { position++; return new Token(TokenKind.Close, "}"); }
            if (text[position] == '"') return new Token(TokenKind.Value, ReadQuoted());
            var start = position;
            while (position < text.Length && !char.IsWhiteSpace(text[position]) && text[position] is not '{' and not '}') position++;
            return new Token(TokenKind.Value, text[start..position]);
        }

        private string ReadQuoted()
        {
            position++;
            var value = new StringBuilder();
            while (position < text.Length)
            {
                var character = text[position++];
                if (character == '"') return value.ToString();
                if (character != '\\') { value.Append(character); continue; }
                if (position >= text.Length) break;
                value.Append(text[position++] switch { 'n' => '\n', 'r' => '\r', 't' => '\t', '"' => '"', '\\' => '\\', var other => other });
            }
            throw new InvalidDataException("Quoted KeyValues string is not closed.");
        }

        private void SkipTrivia()
        {
            while (position < text.Length)
            {
                if (char.IsWhiteSpace(text[position])) { position++; continue; }
                if (position + 1 < text.Length && text[position] == '/' && text[position + 1] == '/')
                {
                    position += 2;
                    while (position < text.Length && text[position] != '\n') position++;
                    continue;
                }
                break;
            }
        }
    }

    private sealed record Token(TokenKind Kind, string Value);
    private enum TokenKind { Value, Open, Close }
}
