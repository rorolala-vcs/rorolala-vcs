#!/bin/bash

#
#         ██  ██████  ██    ██ ███    ██      ███████ ██   ██
#        ██   ██   ██ ██    ██ ████   ██      ██      ██   ██
#       ██    ██████  ██    ██ ██ ██  ██      ███████ ███████
#      ██     ██   ██ ██    ██ ██  ██ ██           ██ ██   ██
#  ██ ██      ██   ██  ██████  ██   ████  ██  ███████ ██   ██
#
#  You can go to [https://catilgrass.github.io/run] to install it
#                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

# Version: 0.1.1

cd "$(dirname "$0")" || exit 1

declare -A tools

for file in .run/src/bin/*.sh; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .sh)
        tools["$name"]="sh"
    fi
done

for file in .run/src/bin/*; do
    if [ -f "$file" ]; then
        name=$(basename "$file")
        if [[ ! "$name" == *.* ]]; then
            tools["$name"]="binary"
        fi
    fi
done

for file in .run/src/bin/*.cs; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .cs)
        tools["$name"]="cs"
    fi
done

for file in .run/src/bin/*.go; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .go)
        tools["$name"]="go"
    fi
done

for file in .run/src/bin/*.nim; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .nim)
        tools["$name"]="nim"
    fi
done

for file in .run/src/bin/*.pl; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .pl)
        tools["$name"]="pl"
    fi
done

for file in .run/src/bin/*.py; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .py)
        tools["$name"]="py"
    fi
done

for file in .run/src/bin/*.rb; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .rb)
        tools["$name"]="rb"
    fi
done

for file in .run/src/bin/*.rs; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .rs)
        tools["$name"]="rs"
    fi
done

for file in .run/src/bin/*.zig; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .zig)
        tools["$name"]="zig"
    fi
done

if [ $# -eq 0 ]; then
    sorted_names=($(
        for name in "${!tools[@]}"; do
            first_char="${name:0:1}"
            if [[ "$first_char" =~ [A-Z] ]]; then
                case_pri="0"
            else
                case_pri="1"
            fi
            if [ "${tools[$name]}" = "sh" ]; then
                lang_pri="0"
            else
                lang_pri="1"
            fi
            echo "$case_pri$lang_pri $name"
        done | sort | while read -r _ n; do echo "$n"; done
    ))
    total=${#sorted_names[@]}
    num_w=${#total}

    max_name=0
    for name in "${sorted_names[@]}"; do
        len=${#name}
        ((len > max_name)) && max_name=$len
    done

    inner_w=$((2 + num_w + 1 + 1 + max_name + 2 + 6 + 1 + 2))
    ((inner_w < 38)) && inner_w=38

    title="./run.sh <NUMBER/NAME> [ARGS...]"
    title_len=${#title}
    dash_total=$((inner_w - 2 - title_len))
    dash_left=$((dash_total / 2))
    dash_right=$((dash_total - dash_left))

    echo "┌$(printf '─%.0s' $(seq 1 $dash_left)) $title $(printf '─%.0s' $(seq 1 $dash_right))┐"
    printf "│%*s│\n" $inner_w ""

    i=1
    for name in "${sorted_names[@]}"; do
        type="${tools[$name]}"
        case "$type" in
            sh) lang="Shell";;
            binary) lang="Binary";;
            cs) lang="C#";;
            go) lang="Go";;
            nim) lang="Nim";;
            pl) lang="Perl";;
            py) lang="Python";;
            rb) lang="Ruby";;
            rs) lang="Rust";;
            zig) lang="Zig";;
        esac
        display_name=$(echo "$name" | tr '_-' '  ')
        entry=$(printf "  %-*d) %-*s [%s]" $num_w $i $max_name "$display_name" $lang)
        printf "│%-*s│\n" $inner_w "$entry"
        i=$((i + 1))
    done

    printf "│%*s│\n" $inner_w ""
    echo "└$(printf '─%.0s' $(seq 1 $inner_w))┘"
    exit 1
fi

target_name="$1"
shift

if [[ "$target_name" =~ ^[0-9]+$ ]]; then
    sorted=($(
        for name in "${!tools[@]}"; do
            first_char="${name:0:1}"
            if [[ "$first_char" =~ [A-Z] ]]; then
                case_pri="0"
            else
                case_pri="1"
            fi
            if [ "${tools[$name]}" = "sh" ]; then
                lang_pri="0"
            else
                lang_pri="1"
            fi
            echo "$case_pri$lang_pri $name"
        done | sort | while read -r _ n; do echo "$n"; done
    ))
    idx=$((target_name - 1))
    if [ "$idx" -ge 0 ] && [ "$idx" -lt "${#sorted[@]}" ]; then
        target_name="${sorted[$idx]}"
    else
        echo "Error: invalid number '$target_name', valid range is 1-${#sorted[@]}"
        exit 1
    fi
fi

if [ -z "${tools[$target_name]}" ]; then
    normalized_user=$(echo "$target_name" | tr '[:upper:]' '[:lower:]' | sed 's/[_. -]//g')
    found=""
    for existing_name in "${!tools[@]}"; do
        normalized_existing=$(echo "$existing_name" | tr '[:upper:]' '[:lower:]' | sed 's/[_. -]//g')
        if [ "$normalized_user" = "$normalized_existing" ]; then
            found="$existing_name"
            break
        fi
    done
    if [ -n "$found" ]; then
        target_name="$found"
    else
        echo "Error: target '$target_name' does not exist"
        exit 1
    fi
fi

type="${tools[$target_name]}"

case "$type" in
    sh)
        chmod +x ".run/src/bin/$target_name.sh"
        ".run/src/bin/$target_name.sh" "$@"
        ;;
    binary)
        chmod +x ".run/src/bin/$target_name"
        ".run/src/bin/$target_name" "$@"
        ;;
    cs)
        temp_dir=".run/target/csproj/$target_name"
        mkdir -p "$temp_dir"
        cat > "$temp_dir/Directory.Build.props" <<'PROPS'
<Project>
  <PropertyGroup>
    <BaseOutputPath>$(MSBuildThisFileDirectory)../../csharp/bin</BaseOutputPath>
    <BaseIntermediateOutputPath>$(MSBuildThisFileDirectory)../../csharp/obj</BaseIntermediateOutputPath>
  </PropertyGroup>
</Project>
PROPS
        cat > "$temp_dir/$target_name.csproj" <<'CSPROJ'
<Project Sdk="Microsoft.NET.Sdk">

    <PropertyGroup>
        <OutputType>Exe</OutputType>
        <TargetFramework>net8.0</TargetFramework>
        <ImplicitUsings>enable</ImplicitUsings>
        <Nullable>enable</Nullable>
    </PropertyGroup>

</Project>
CSPROJ
        cp ".run/src/bin/$target_name.cs" "$temp_dir/Program.cs"
        dotnet run --project "$temp_dir/$target_name.csproj" -- "$@"
        ;;
    go)
        go run ".run/src/bin/$target_name.go" "$@"
        ;;
    nim)
        nim r --hints:off ".run/src/bin/$target_name.nim" "$@"
        ;;
    pl)
        perl ".run/src/bin/$target_name.pl" "$@"
        ;;
    py)
        python ".run/src/bin/$target_name.py" "$@"
        ;;
    rb)
        ruby ".run/src/bin/$target_name.rb" "$@"
        ;;
    rs)
        if [ ! -f ".run/Cargo.toml" ]; then
            cat > ".run/Cargo.toml" <<'EOF'
[package]
name = "run_rust"
version = "0.1.0"
edition = "2024"

[workspace]

[dependencies]
EOF
        fi
        cargo build --manifest-path ".run/Cargo.toml" --target-dir ".run/target" --bin "$target_name" --quiet
        ".run/target/debug/$target_name" "$@"
        ;;
    zig)
        zig run ".run/src/bin/$target_name.zig" "$@"
        ;;
esac
