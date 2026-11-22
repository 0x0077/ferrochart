// A minimal PNG writer for RGBA8 images (no dependencies beyond the BCL).

using System.Buffers.Binary;
using System.IO.Compression;
using System.Text;

namespace FerroChart.Example;

public static class Png
{
    private static readonly uint[] CrcTable = Enumerable.Range(0, 256).Select(n =>
    {
        var c = (uint)n;
        for (var k = 0; k < 8; k++) c = (c & 1) != 0 ? 0xEDB88320u ^ (c >> 1) : c >> 1;
        return c;
    }).ToArray();

    public static void Write(string path, byte[] rgba, int width, int height)
    {
        using var file = File.Create(path);
        file.Write([0x89, (byte)'P', (byte)'N', (byte)'G', 0x0D, 0x0A, 0x1A, 0x0A]);

        var header = new byte[13];
        BinaryPrimitives.WriteInt32BigEndian(header, width);
        BinaryPrimitives.WriteInt32BigEndian(header.AsSpan(4), height);
        header[8] = 8; // bit depth
        header[9] = 6; // RGBA
        Chunk(file, "IHDR", header);

        using var data = new MemoryStream();
        using (var z = new ZLibStream(data, CompressionLevel.Fastest, leaveOpen: true))
        {
            for (var y = 0; y < height; y++)
            {
                z.WriteByte(0); // no filter
                z.Write(rgba, y * width * 4, width * 4);
            }
        }
        Chunk(file, "IDAT", data.ToArray());
        Chunk(file, "IEND", []);
    }

    private static void Chunk(Stream s, string type, byte[] body)
    {
        Span<byte> word = stackalloc byte[4];
        BinaryPrimitives.WriteInt32BigEndian(word, body.Length);
        s.Write(word);
        var tag = Encoding.ASCII.GetBytes(type);
        s.Write(tag);
        s.Write(body);
        var crc = 0xFFFFFFFFu;
        foreach (var b in tag.Concat(body)) crc = CrcTable[(crc ^ b) & 0xFF] ^ (crc >> 8);
        BinaryPrimitives.WriteUInt32BigEndian(word, crc ^ 0xFFFFFFFFu);
        s.Write(word);
    }
}
