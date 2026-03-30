#ifndef CONCERTO_H
#define CONCERTO_H

/*
 * concerto.h
 *
 * C header for the concerto-rs FFI layer.
 *
 * build first:
 *   cargo build --release -p concerto-ffi
 *
 * memory rules:
 *   - returned char* values belong to the library
 *   - free them with concerto_free_string()
 *   - ConcertoModelManager* must be freed with concerto_model_manager_free()
 */

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>

typedef struct ConcertoModelManager ConcertoModelManager;

ConcertoModelManager *concerto_model_manager_new(void);
void concerto_model_manager_free(ConcertoModelManager *mm);

char *concerto_add_model(ConcertoModelManager *mm, const char *json);

char *concerto_validate_instance(
    const ConcertoModelManager *mm,
    const char *instance_json,
    const char *type_name
);

void concerto_free_string(char *s);

#ifdef __cplusplus
}
#endif

#endif
