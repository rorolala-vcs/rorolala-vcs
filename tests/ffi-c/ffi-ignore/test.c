/*
 * The ignore rules across the ABI: loaded from a directory, asked about paths, and released.
 *
 * What a caller leans on. A directory of rule files loads into an opaque handle, and a path the rules
 * match is answered as ignored while a path they do not match is not. The order the files apply in is
 * part of the answer, not of the loader alone: a rule fetched from a Vault sits deeper than the
 * Workspace's own and yields to it, and a rule in a later-sorting file decides over an earlier one.
 * A directory that is not there is an empty set rather than a failure, and what is handed out is
 * given back rather than leaked.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#include "rorolala_ffi.h"

#include "harness.h"

/* Writes `text` to `path`, which its parents are expected to hold already. */
static void put(const char *path, const char *text) {
    FILE *file = fopen(path, "w");
    if (file == NULL) {
        perror(path);
        exit(2);
    }

    fputs(text, file);
    fclose(file);
}

/* Makes `path`, which must not be there yet, and stops the program when it cannot. */
static void make_dir(const char *path) {
    if (mkdir(path, 0755) != 0) {
        perror(path);
        exit(2);
    }
}

int main(void) {
    const char *tmp = getenv("TMPDIR");
    if (tmp == NULL) {
        tmp = "/tmp";
    }

    /* A directory of this run's own, where a Workspace's rules and its tree both sit. */
    char root[512];
    snprintf(root, sizeof root, "%s/rola-ffi-ignore-XXXXXX", tmp);
    if (mkdtemp(root) == NULL) {
        perror("mkdtemp");
        return 2;
    }

    char rules[600];
    snprintf(rules, sizeof rules, "%s/rules", root);
    make_dir(rules);

    /*
     * The Workspace's own rules: `*.mb` ignores Maya files but keeps one, and the Vault's copy of that
     * rule sits deeper — so the Workspace's two lines are what stands.
     */
    char maya[700];
    snprintf(maya, sizeof maya, "%s/maya", rules);
    put(maya, "*.mb\n!keep.mb\n");

    char vaults[800];
    snprintf(vaults, sizeof vaults, "%s/vaults", rules);
    make_dir(vaults);

    char origin[900];
    snprintf(origin, sizeof origin, "%s/vaults/origin", rules);
    make_dir(origin);

    char fetched[1000];
    snprintf(fetched, sizeof fetched, "%s/vaults/origin/maya", rules);
    put(fetched, "*.mb\n");

    /*
     * A second set beside the first: two files of one depth, and the later-sorting one decides.
     */
    char later[600];
    snprintf(later, sizeof later, "%s/later", root);
    make_dir(later);

    char earlier[700];
    snprintf(earlier, sizeof earlier, "%s/a", later);
    put(earlier, "*.mb\n");

    char sooner[800];
    snprintf(sooner, sizeof sooner, "%s/b", later);
    put(sooner, "!*.mb\n");

    RolaIgnoreRules *set = load_rola_ignore_rules(rules, root);
    want(set != NULL, "a directory of rules loads");

    if (set != NULL) {
        char path[1200];

        snprintf(path, sizeof path, "%s/scenes/a.mb", root);
        want(rola_ignore_rules_is_ignore(set, path, false), "a rule ignores what it matches");

        snprintf(path, sizeof path, "%s/scenes/a.ma", root);
        want(!rola_ignore_rules_is_ignore(set, path, false), "a path no rule matches is not ignored");

        /* The Vault's deeper `*.mb` was applied first, and the Workspace's negation last: it stands. */
        snprintf(path, sizeof path, "%s/scenes/keep.mb", root);
        want(
            !rola_ignore_rules_is_ignore(set, path, false),
            "a Workspace rule decides over the one it fetched from a Vault"
        );

        free_rola_ignore_rules(set);
    }

    RolaIgnoreRules *ordered = load_rola_ignore_rules(later, root);
    want(ordered != NULL, "a set of two files loads");

    if (ordered != NULL) {
        char path[1200];
        snprintf(path, sizeof path, "%s/scenes/a.mb", root);
        want(
            !rola_ignore_rules_is_ignore(ordered, path, false),
            "the file whose name sorts later decides"
        );

        free_rola_ignore_rules(ordered);
    }

    /* A directory that is not there is an empty set, and it ignores nothing. */
    char absent[600];
    snprintf(absent, sizeof absent, "%s/absent", root);

    RolaIgnoreRules *empty = load_rola_ignore_rules(absent, root);
    want(empty != NULL, "a directory no rules were written to loads as an empty set");

    if (empty != NULL) {
        char path[1200];
        snprintf(path, sizeof path, "%s/scenes/a.mb", root);
        want(!rola_ignore_rules_is_ignore(empty, path, false), "an empty set ignores nothing");

        free_rola_ignore_rules(empty);
    }

    remove_tree(root);

    return finish();
}
