using RolaSharp;
using Xunit;

namespace RolaSharp.IntegrationTests;

public sealed class ScopeTests
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
