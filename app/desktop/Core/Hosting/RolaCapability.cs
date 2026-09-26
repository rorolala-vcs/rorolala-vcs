using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// What Rorolala can do, injected over the C ABI.
/// </summary>
/// <remarks>
/// The interface is declared in the contract and is bound to the exports of the C ABI. Until those
/// bindings are written the interface states nothing, and this is what implements it, so a plugin is
/// always handed a host whose <c>Rola</c> is present even when there is nothing to call on it yet.
/// A plugin calls this rather than invoking the command line: shelling out is not a substitute for
/// an export, and is not permitted (Section 13).
/// </remarks>
internal sealed class RolaCapability : IRola
{
}
