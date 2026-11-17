namespace RolaSharp.V2;

/// <summary>A key file a user or a Vault holds, and the name it goes by.</summary>
/// <param name="Name">The name, which is the stem of the key file.</param>
/// <param name="File">Where the key file sits.</param>
public record Key(string Name, FileInfo File);

/// <summary>A public key: the half of a key pair that is meant to be shared.</summary>
/// <param name="Name">The name, which is the stem of the key file.</param>
/// <param name="File">Where the public key file sits.</param>
public sealed record PublicKey(string Name, FileInfo File) : Key(Name, File);

/// <summary>A private key: the half of a key pair a run signs with.</summary>
/// <param name="Name">The name, which is the stem of the key file.</param>
/// <param name="File">Where the private key file sits.</param>
public sealed record PrivateKey(string Name, FileInfo File) : Key(Name, File);

/// <summary>An account: the private key a run signs with, and the public key beside it.</summary>
/// <param name="Name">The name the account goes by.</param>
/// <param name="Private">Its private key.</param>
/// <param name="Public">Its public key, when one sits beside the private one.</param>
public sealed record Account(string Name, PrivateKey Private, PublicKey? Public);
