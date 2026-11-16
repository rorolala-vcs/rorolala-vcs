/*
 * The store across the ABI: what it holds, what comes out of it, and the three ways a read fails.
 *
 * The fixture Workspace holds one small file in its store, made by the runner. What a caller leans
 * on is the listing of keys the commands write, the extraction into a path of its own, and the tags
 * that tell "nothing stored under that key" from "no store here at all".
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "rorolala_ffi.h"

#include "harness.h"

/* The fixture paths and key the runner names in the environment. */
static const char *fixture(const char *name) {
    const char *value = getenv(name);
    return value == NULL ? "" : value;
}

/* Releases the error a failed store read carried, whatever case it is. */
static void released(RorolalaResult answer) {
    if (answer.tag == RorolalaResult_Err && answer.payload != NULL) {
        free_rola_storage_error(answer.payload);
    }
}

int main(void) {
    const char *workspace = fixture("ROLA_WORKSPACE");
    const char *key = fixture("ROLA_STORAGE_KEY");
    const char *scratch = fixture("ROLA_SCRATCH");

    /* The listing is the keys the store holds, and the fixture put one there. */
    RorolalaResult objects = rola_storage_objects_json(workspace);
    want(objects.tag == RorolalaResult_Ok, "a Workspace's store lists its objects");
    if (objects.tag == RorolalaResult_Ok) {
        const char *json = objects.payload;
        want(json != NULL && strstr(json, "\"keys\"") != NULL, "the listing names keys");
        want(key[0] != '\0' && json != NULL && strstr(json, key) != NULL,
             "the content the fixture stored is listed under its key");
        free_string(objects.payload);
    } else {
        released(objects);
    }

    /* Manifests are listed the same way; a small object is stored whole, so it need not be one. */
    RorolalaResult manifests = rola_storage_manifests_json(workspace);
    want(manifests.tag == RorolalaResult_Ok, "a Workspace's store lists its manifests");
    if (manifests.tag == RorolalaResult_Ok) {
        const char *json = manifests.payload;
        want(json != NULL && strstr(json, "\"keys\"") != NULL, "the manifest listing names keys");
        free_string(manifests.payload);
    } else {
        released(manifests);
    }

    /* One content is taken out into a path the caller names, once. */
    char target[512];
    snprintf(target, sizeof target, "%s/extracted.bin", scratch);

    RorolalaResult extracted = rola_storage_extract(workspace, key, target);
    want(extracted.tag == RorolalaResult_Ok, "the stored content is extracted");
    if (extracted.tag == RorolalaResult_Err) {
        released(extracted);
    }

    FILE *file = fopen(target, "rb");
    want(file != NULL, "the content is where it was asked for");
    if (file != NULL) {
        char read[64];
        size_t got = fread(read, 1, sizeof read, file);
        fclose(file);
        want(got == strlen("fixture content"), "the content is the length it was stored at");
        want(got == strlen("fixture content") && memcmp(read, "fixture content", got) == 0,
             "the content is what was stored");
    }

    /* The same content written twice into one place is refused rather than written over. */
    RorolalaResult again = rola_storage_extract(workspace, key, target);
    want(again.tag == RorolalaResult_Err, "a path already there is refused");
    if (again.tag == RorolalaResult_Err) {
        const RolaStorageError *error = again.payload;
        want(error->tag == RolaStorageError_Exists, "it is reported as Exists");
        released(again);
    }

    /* A key nothing is stored under is told apart from a store that cannot be read. */
    const char *missing = "blake3:1111111111111111111111111111111111111111111111111111111111111111";
    RorolalaResult absent = rola_storage_extract(workspace, missing, target);
    want(absent.tag == RorolalaResult_Err, "a key nothing is stored under is a failure");
    if (absent.tag == RorolalaResult_Err) {
        const RolaStorageError *error = absent.payload;
        want(error->tag == RolaStorageError_NotFound, "it is reported as NotFound");
        released(absent);
    }

    /* A directory no place holds has no store to read. */
    RorolalaResult nowhere = rola_storage_objects_json("/nonexistent/rola/store");
    want(nowhere.tag == RorolalaResult_Err, "a directory outside every place is a failure");
    if (nowhere.tag == RorolalaResult_Err) {
        const RolaStorageError *error = nowhere.payload;
        want(error->tag == RolaStorageError_NoStore, "it is reported as NoStore");
        released(nowhere);
    }

    remove(target);

    return finish();
}
