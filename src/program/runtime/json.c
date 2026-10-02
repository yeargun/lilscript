/* Checked portable JSON values: exact primitive spelling, UTF-16 quoting and
   record key order. Typed generated array recipes share this builder. */
static LS_NATIVE_UNUSED inline void ls_json_quote(ls_string_builder *out, ls_string text) {
    static const char hex[]="0123456789abcdef";
    ls_string_builder_unit(out,'"');
    for(size_t i=0;i<text.length;i++) {
        uint16_t c=text.data[i];
        if(c=='"' || c=='\\') { ls_string_builder_unit(out,'\\'); ls_string_builder_unit(out,c); }
        else if(c==8 || c==9 || c==10 || c==12 || c==13) {
            ls_string_builder_unit(out,'\\'); ls_string_builder_unit(out,c==8?'b':c==9?'t':c==10?'n':c==12?'f':'r');
        } else if(c>=0xD800 && c<=0xDBFF && i+1<text.length && text.data[i+1]>=0xDC00 && text.data[i+1]<=0xDFFF) {
            ls_string_builder_unit(out,c); ls_string_builder_unit(out,text.data[++i]);
        } else if(c<32 || (c>=0xD800 && c<=0xDFFF)) {
            ls_string_builder_ascii(out,"\\u");
            for(int shift=12;shift>=0;shift-=4) ls_string_builder_unit(out,(uint16_t)hex[(c>>shift)&15]);
        } else ls_string_builder_unit(out,c);
    }
    ls_string_builder_unit(out,'"');
}
static LS_NATIVE_UNUSED inline void ls_json_value(ls_string_builder *out, ls_value value) {
    switch(value.tag) {
    case LS_NULL: ls_string_builder_ascii(out,"null"); break;
    case LS_BOOL: ls_string_builder_ascii(out,value.as.b?"true":"false"); break;
    case LS_INT: {
        char text[16]; snprintf(text,sizeof text,"%ld",(long)value.as.i); ls_string_builder_ascii(out,text); break;
    }
    case LS_FLOAT:
        if(!isfinite(value.as.f)) ls_string_builder_ascii(out,"null");
        else { ls_string text=ls_number_to_string(value.as.f); ls_string_builder_text(out,text); ls_string_release(text); }
        break;
    case LS_STRING: ls_json_quote(out,value.as.s); break;
    default: ls_value_mismatch();
    }
}
static LS_NATIVE_UNUSED inline ls_string ls_json_scalar(ls_value value) {
    ls_string_builder out={0}; ls_json_value(&out,value); return ls_string_builder_finish(&out);
}
static LS_NATIVE_UNUSED inline ls_string ls_json_record(ls_native_object *owner) {
    ls_map *record=(ls_map *)owner;
    ls_record_key *order=ls_record_order(owner);
    ls_string_builder out={0}; ls_string_builder_unit(&out,'{');
    for(size_t i=0;i<record->size;i++) {
        ls_map_entry *entry=&record->entries[order[i].position];
        if(i) ls_string_builder_unit(&out,',');
        ls_json_quote(&out,entry->key.as.s); ls_string_builder_unit(&out,':'); ls_json_value(&out,entry->value);
    }
    free(order); ls_string_builder_unit(&out,'}'); return ls_string_builder_finish(&out);
}
