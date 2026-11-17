using RolaSharp.V2;
using Xunit;

namespace RolaSharp.V2.IntegrationTests;

public sealed class ScoopTests
{
    [Fact]
    public void A_policy_has_a_kebab_case_wire_name()
    {
        Assert.Equal("psd-basic", ChunkingPolicies.WireName(ChunkingPolicy.PsdBasic));
    }

    [Fact]
    public void A_policy_lists_the_containers_it_claims()
    {
        Assert.Contains("psd", ChunkingPolicies.Extensions(ChunkingPolicy.PsdBasic));
    }
}
