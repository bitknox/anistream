using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using System.Text;

namespace Anistream.App;

/// <summary>
/// Tracker tokens in Windows Credential Manager, as generic credentials named
/// <c>anistream/&lt;id&gt;</c>, kept for this user on this machine (CRED_PERSIST_LOCAL_MACHINE).
/// </summary>
[SupportedOSPlatform("windows")]
public static unsafe partial class Credentials
{
    const uint CredTypeGeneric = 1;
    const uint CredPersistLocalMachine = 2;
    const int ErrorNotFound = 1168;

    [StructLayout(LayoutKind.Sequential)]
    struct Credential
    {
        public uint Flags;
        public uint Type;
        public char* TargetName;
        public char* Comment;
        public long LastWritten;
        public uint CredentialBlobSize;
        public byte* CredentialBlob;
        public uint Persist;
        public uint AttributeCount;
        public nint Attributes;
        public char* TargetAlias;
        public char* UserName;
    }

    [LibraryImport("advapi32.dll", EntryPoint = "CredReadW", StringMarshalling = StringMarshalling.Utf16, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool CredRead(string target, uint type, uint flags, out Credential* credential);

    [LibraryImport("advapi32.dll", EntryPoint = "CredWriteW", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool CredWrite(Credential* credential, uint flags);

    [LibraryImport("advapi32.dll", EntryPoint = "CredDeleteW", StringMarshalling = StringMarshalling.Utf16, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool CredDelete(string target, uint type, uint flags);

    [LibraryImport("advapi32.dll", EntryPoint = "CredFree")]
    private static partial void CredFree(void* buffer);

    static string Target(string id) => $"anistream/{id}";

    public static string? Get(string id)
    {
        if (!CredRead(Target(id), CredTypeGeneric, 0, out var cred))
            return null;
        try
        {
            return Encoding.UTF8.GetString(cred->CredentialBlob, (int)cred->CredentialBlobSize);
        }
        finally
        {
            CredFree(cred);
        }
    }

    public static bool Set(string id, string value)
    {
        var blob = Encoding.UTF8.GetBytes(value);
        var target = Target(id);
        fixed (char* name = target)
        fixed (char* user = "anistream")
        fixed (byte* data = blob)
        {
            var cred = new Credential
            {
                Type = CredTypeGeneric,
                TargetName = name,
                CredentialBlobSize = (uint)blob.Length,
                CredentialBlob = data,
                Persist = CredPersistLocalMachine,
                UserName = user,
            };
            return CredWrite(&cred, 0);
        }
    }

    /// <summary>True when it is gone, including when there was none.</summary>
    public static bool Delete(string id) =>
        CredDelete(Target(id), CredTypeGeneric, 0) || Marshal.GetLastPInvokeError() == ErrorNotFound;
}
