/*
 * The book across the ABI: opened, located, and searched.
 *
 * A book is reached the way the index and the version graph are — an owned handle or nothing — and
 * a search answers with a list of hits, each carrying the description and fragments a search
 * result is drawn with. What is worth meeting here is the nesting: a hit is a value of its own, a
 * fragment is a value of its own, and every one of the three is given back with its own `free_*`.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "rorolala_ffi.h"

#include "harness.h"

int main(void) {
    const char *path = getenv("ROLA_BOOK");
    const char *beside = getenv("ROLA_BOOK_LOCATE");

    want(path != NULL, "the suite names the book it made");
    want(beside != NULL, "the suite names the directory the book is located from");
    if (path == NULL || beside == NULL) {
        return finish();
    }

    RorolalaResult opened = open_rola_book_search(path);
    want(opened.tag == RorolalaResult_Ok, "a book opens");
    if (opened.tag != RorolalaResult_Ok) {
        if (opened.payload != NULL) {
            free_rola_book_error(opened.payload);
        }
        return finish();
    }

    RolaBookSearch *book = opened.payload;
    want(book != NULL, "opening answers with a book rather than with nothing");
    if (book == NULL) {
        return finish();
    }

    RolaBookSearch *located = locate_rola_book_search(beside);
    want(located != NULL, "the book is located by walking up from a program's directory");
    if (located != NULL) {
        free_rola_book_search(located);
    }

    want(count_rola_book_articles(book) == 2, "the book holds the two articles it was made of");

    RolaBookResults *results = search_rola_book(book, "needle", 10);
    want(results != NULL, "a search answers with a list rather than failing");

    if (results != NULL) {
        want(rola_book_results_count(results) == 1, "only the article that says needle is found");

        RolaBookHit *hit = rola_book_results_hit(results, 0);
        want(hit != NULL, "the article that matched is handed out");

        if (hit != NULL) {
            char *title = rola_book_hit_title(hit);
            want(title != NULL && strcmp(title, "The Needle Article") == 0, "a hit carries its title");
            free_string(title);

            char *description = rola_book_hit_description(hit);
            want(
                description != NULL && strstr(description, "haystack") != NULL,
                "a hit carries its description"
            );
            free_string(description);

            want(rola_book_hit_score(hit) > 0.0, "a hit carries a score");
            want(rola_book_hit_snippet_count(hit) >= 1, "a hit carries the fragments that matched");

            RolaBookSnippet *snippet = rola_book_hit_snippet(hit, 0);
            want(snippet != NULL, "a fragment is handed out");

            if (snippet != NULL) {
                char *field = rola_book_snippet_field(snippet);
                want(field != NULL && strcmp(field, "content") == 0, "a fragment names its field");
                free_string(field);

                char *text = rola_book_snippet_text(snippet);
                want(text != NULL && strstr(text, "needle") != NULL, "a fragment holds the word");
                free_string(text);

                want(
                    rola_book_snippet_range_count(snippet) >= 1,
                    "a fragment marks where the word is"
                );
                want(
                    rola_book_snippet_range_length(snippet, 0) > 0,
                    "a marked range covers at least one character"
                );

                free_rola_book_snippet(snippet);
            }

            free_rola_book_hit(hit);
        }

        want(rola_book_results_hit(results, 99) == NULL, "past the end of the list is nothing");

        free_rola_book_results(results);
    }

    RorolalaResult refused = open_rola_book_search("/nonexistent/rola-ffi-book/builtin.book");
    want(refused.tag == RorolalaResult_Err, "a file that is not there is refused");
    if (refused.tag == RorolalaResult_Err && refused.payload != NULL) {
        free_rola_book_error(refused.payload);
    }

    free_rola_book_search(book);

    return finish();
}
