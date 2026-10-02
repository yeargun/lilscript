/* Expression-temporary ownership. A product box embeds its link, so there is
   no side allocation or process-lifetime arena. The generated activation owns
   this chain until its statement has copied/retained every escaping result. */
typedef struct ls_native_temporary {
    struct ls_native_temporary *next;
    ls_native_object *owner;
} ls_native_temporary;
static LS_NATIVE_UNUSED inline void ls_native_temporary_push(ls_native_temporary **head, ls_native_temporary *entry, ls_native_object *owner) {
    entry->next=*head; entry->owner=owner; *head=entry;
}
static LS_NATIVE_UNUSED inline void ls_native_temporaries_clear(ls_native_temporary **head) {
    while (*head) {
        ls_native_temporary *entry=*head;
        ls_native_object *owner=entry->owner;
        *head=entry->next;
        ls_native_release(owner);
    }
}
