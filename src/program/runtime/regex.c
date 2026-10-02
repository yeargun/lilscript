/* Typed ECMAScript RegExp operations over the pinned standalone libregexp.
   The object owns its bytecode, escaped source and canonical flags. Operations
   borrow it; lastIndex is shared by all aliases. No JavaScript VM is embedded. */
typedef struct {
    uintptr_t stack_origin;
    uint64_t polls;
    bool stack_exhausted, work_exhausted;
} ls_regex_context;
static int lre_check_stack_overflow(void *opaque, size_t extra) {
    ls_regex_context *context=opaque;
    /* ASan may move automatic variables onto a fake stack. Frame addresses
       still measure the actual native stack consumed by the parser. */
    uintptr_t current=(uintptr_t)__builtin_frame_address(0), origin=context->stack_origin;
    size_t used=current>origin?current-origin:origin-current;
    if(extra>LS_NATIVE_REGEX_STACK_LIMIT || used>LS_NATIVE_REGEX_STACK_LIMIT-extra) {
        context->stack_exhausted=true; return 1;
    }
    return 0;
}
static int lre_check_timeout(void *opaque) {
    ls_regex_context *context=opaque;
    if(LS_NATIVE_REGEX_POLL_LIMIT && context->polls++==LS_NATIVE_REGEX_POLL_LIMIT) {
        context->work_exhausted=true; return 1;
    }
    return 0;
}
static void *lre_realloc(void *opaque, void *pointer, size_t size) {
    (void)opaque;
    if(!size) { free(pointer); return NULL; }
    return realloc(pointer,size);
}
typedef struct {
    ls_native_object owner;
    uint8_t *bytecode;
    ls_string source, flags;
    double last_index;
    int bits;
} ls_regex;
static LS_NATIVE_UNUSED void ls_regex_destroy(ls_native_object *owner) {
    ls_regex *regex=(ls_regex *)owner;
    free(regex->bytecode); ls_string_release(regex->source); ls_string_release(regex->flags);
}
static LS_NATIVE_UNUSED void ls_regex_trace(ls_native_object *owner, ls_native_visit visit, void *context) {
    ls_regex *regex=(ls_regex *)owner;
    visit(regex->source.owner,context); visit(regex->flags.owner,context);
}
static LS_NATIVE_UNUSED ls_string ls_string_utf8(const char *bytes) {
    ls_string_builder out={0}; const uint8_t *at=(const uint8_t *)bytes;
    size_t remaining=strlen(bytes);
    while(remaining) {
        const uint8_t *next; int point=unicode_from_utf8(at,(int)(remaining>6?6:remaining),&next);
        if(point<0) { point=0xFFFD; next=at+1; }
        remaining-=(size_t)(next-at); at=next;
        ls_string_builder_point(&out,(uint32_t)point);
    }
    return ls_string_builder_finish(&out);
}
static LS_NATIVE_UNUSED void ls_regex_error(const char *name, const char *message) {
    ls_native_object *error=ls_record_new();
    ls_string nk=ls_string_ascii("name",4),mk=ls_string_ascii("message",7);
    ls_string n=ls_string_ascii(name,strlen(name)),m=ls_string_utf8(message);
    ls_record_set(error,nk,ls_value_string(n)); ls_record_set(error,mk,ls_value_string(m));
    ls_native_throw(ls_value_object(error));
    ls_string_release(nk); ls_string_release(mk); ls_string_release(n); ls_string_release(m);
    ls_native_release(error);
}
static LS_NATIVE_UNUSED ls_string ls_regex_source(ls_string pattern) {
    if(!pattern.length) return ls_string_ascii("(?:)",4);
    ls_string_builder out={0}; bool character_class=false;
    for(size_t i=0;i<pattern.length;i++) {
        uint16_t c=pattern.data[i];
        if(c=='\\' && i+1<pattern.length && pattern.data[i+1]!='\n' && pattern.data[i+1]!='\r'
            && pattern.data[i+1]!=0x2028 && pattern.data[i+1]!=0x2029) {
            ls_string_builder_unit(&out,c); ls_string_builder_unit(&out,pattern.data[++i]);
        } else if(c=='[') { character_class=true; ls_string_builder_unit(&out,c); }
        else if(c==']') { character_class=false; ls_string_builder_unit(&out,c); }
        else if(c=='/' && !character_class) ls_string_builder_ascii(&out,"\\/");
        else if(c=='\n') ls_string_builder_ascii(&out,"\\n");
        else if(c=='\r') ls_string_builder_ascii(&out,"\\r");
        else if(c==0x2028) ls_string_builder_ascii(&out,"\\u2028");
        else if(c==0x2029) ls_string_builder_ascii(&out,"\\u2029");
        else if(c=='\\' && i+1<pattern.length) { /* escaped line terminator */ }
        else ls_string_builder_unit(&out,c);
    }
    return ls_string_builder_finish(&out);
}
static LS_NATIVE_UNUSED ls_native_object *ls_regex_new(ls_string pattern, ls_string flags) {
    const char *letters="dgimsuvy";
    const int values[]={LRE_FLAG_INDICES,LRE_FLAG_GLOBAL,LRE_FLAG_IGNORECASE,LRE_FLAG_MULTILINE,
        LRE_FLAG_DOTALL,LRE_FLAG_UNICODE,LRE_FLAG_UNICODE_SETS,LRE_FLAG_STICKY};
    int bits=0;
    for(size_t i=0;i<flags.length;i++) {
        const char *letter=flags.data[i] && flags.data[i]<128?strchr(letters,(char)flags.data[i]):NULL;
        if(!letter || (bits&values[letter-letters])) {
            ls_regex_error("SyntaxError","Invalid regular expression flags"); return NULL;
        }
        bits|=values[letter-letters];
    }
    if((bits&LRE_FLAG_UNICODE) && (bits&LRE_FLAG_UNICODE_SETS)) {
        ls_regex_error("SyntaxError","Regular expression flags u and v cannot be combined"); return NULL;
    }
    DynBuf encoded; dbuf_init(&encoded);
    for(size_t at=0;at<pattern.length;) {
        uint8_t bytes[UTF8_CHAR_LEN_MAX];
        int count=unicode_to_utf8(bytes,ls_string_next_point(pattern,&at));
        dbuf_put(&encoded,bytes,(size_t)count);
    }
    if(dbuf_error(&encoded)) {
        dbuf_free(&encoded); ls_native_raise_error("RangeError","Regular expression allocation failed"); return NULL;
    }
    /* libregexp reads a sentinel past the explicit length. */
    dbuf_putc(&encoded,0);
    if(dbuf_error(&encoded)) {
        dbuf_free(&encoded); ls_native_raise_error("RangeError","Regular expression allocation failed"); return NULL;
    }
    char error[256]; int size;
    ls_regex_context context={(uintptr_t)__builtin_frame_address(0),0,false,false};
    uint8_t *bytecode=lre_compile(&size,error,sizeof error,(char *)encoded.buf,encoded.size-1,bits,&context);
    dbuf_free(&encoded);
    if(!bytecode) {
        ls_regex_error(context.stack_exhausted?"RangeError":"SyntaxError",error); return NULL;
    }
    ls_regex *regex=ls_native_allocate(sizeof *regex,ls_regex_destroy,ls_regex_trace);
    regex->bytecode=bytecode; regex->bits=bits; regex->last_index=0;
    regex->source=ls_regex_source(pattern);
    ls_string_builder out={0};
    for(size_t i=0;i<8;i++) if(bits&values[i]) ls_string_builder_unit(&out,(uint16_t)letters[i]);
    regex->flags=ls_string_builder_finish(&out);
    return &regex->owner;
}
typedef struct { uint8_t **capture; const uint16_t *data; int count; } ls_regex_match;
static const uint16_t ls_regex_empty_input[1]={0};
static LS_NATIVE_UNUSED bool ls_regex_exec(ls_regex *regex, ls_string input, ls_regex_match *match,
                                          ls_regex_context *context) {
    if(input.length>(size_t)INT32_MAX/2) { ls_native_raise_error("RangeError","Regular expression input exceeds the runtime limit"); return false; }
    bool stateful=(regex->bits&(LRE_FLAG_GLOBAL|LRE_FLAG_STICKY))!=0;
    double start=stateful?regex->last_index:0;
    if(!(start>0)) start=0; else start=floor(start);
    if(start>(double)input.length) { if(stateful) regex->last_index=0; return false; }
    match->count=lre_get_capture_count(regex->bytecode);
    match->capture=calloc((size_t)lre_get_alloc_count(regex->bytecode),sizeof *match->capture);
    if(!match->capture) { ls_native_raise_error("RangeError","Regular expression allocation failed"); return false; }
    match->data=input.length?input.data:ls_regex_empty_input;
    int status=lre_exec(match->capture,regex->bytecode,(const uint8_t *)match->data,(int)start,(int)input.length,1,context);
    if(status<0) {
        ls_native_raise_error("RangeError",status==LRE_RET_TIMEOUT?"Regular expression work limit exceeded":"Regular expression allocation failed");
        return false;
    }
    if(!status) { if(stateful) regex->last_index=0; return false; }
    if(stateful) regex->last_index=(double)((const uint16_t *)match->capture[1]-match->data);
    return true;
}
static LS_NATIVE_UNUSED bool ls_regex_test(ls_native_object *owner, ls_string input) {
    ls_regex_context context={(uintptr_t)__builtin_frame_address(0),0,false,false};
    ls_regex_match match={0}; bool found=ls_regex_exec((ls_regex *)owner,input,&match,&context);
    free(match.capture); return found;
}
static LS_NATIVE_UNUSED int32_t ls_regex_search(ls_string input, ls_native_object *owner) {
    ls_regex *regex=(ls_regex *)owner; double previous=regex->last_index; regex->last_index=0;
    ls_regex_context context={(uintptr_t)__builtin_frame_address(0),0,false,false};
    ls_regex_match match={0}; bool found=ls_regex_exec(regex,input,&match,&context);
    /* Search restores even negative zero after a normal execution. */
    if(!ls_native_raised) regex->last_index=previous;
    int32_t result=found?(int32_t)((const uint16_t *)match.capture[0]-match.data):-1;
    free(match.capture); return result;
}
static LS_NATIVE_UNUSED void ls_regex_capture_text(ls_string_builder *out, ls_regex_match *match, int index) {
    if(!match->capture[2*index]) return;
    const uint16_t *start=(const uint16_t *)match->capture[2*index];
    const uint16_t *end=(const uint16_t *)match->capture[2*index+1];
    ls_string_builder_text(out,(ls_string){start,(size_t)(end-start),NULL});
}
static LS_NATIVE_UNUSED void ls_regex_substitute(ls_string_builder *out, ls_regex *regex,
    ls_regex_match *match, ls_string input, ls_string replacement, size_t start, size_t end) {
    const char *names=lre_get_groupnames(regex->bytecode);
    for(size_t i=0;i<replacement.length;i++) {
        uint16_t c=replacement.data[i];
        if(c!='$' || i+1==replacement.length) { ls_string_builder_unit(out,c); continue; }
        uint16_t next=replacement.data[i+1];
        if(next=='$') { ls_string_builder_unit(out,'$'); i++; }
        else if(next=='&') { ls_regex_capture_text(out,match,0); i++; }
        else if(next=='`') { ls_string_builder_text(out,(ls_string){match->data,start,NULL}); i++; }
        else if(next=='\'') { ls_string_builder_text(out,(ls_string){match->data+end,input.length-end,NULL}); i++; }
        else if(next>='0' && next<='9') {
            int index=next-'0'; size_t digits=1;
            if(i+2<replacement.length && replacement.data[i+2]>='0' && replacement.data[i+2]<='9') {
                int pair=10*index+replacement.data[i+2]-'0';
                if(pair>0 && pair<match->count) { index=pair; digits=2; }
            }
            if(index>0 && index<match->count) { ls_regex_capture_text(out,match,index); i+=digits; }
            else ls_string_builder_unit(out,'$');
        } else if(next=='<' && names) {
            size_t close=i+2; while(close<replacement.length && replacement.data[close]!='>') close++;
            if(close==replacement.length) { ls_string_builder_unit(out,'$'); continue; }
            ls_string key={replacement.data+i+2,close-i-2,NULL}; const char *name=names;
            for(int group=1;group<match->count;group++) {
                ls_string decoded=ls_string_utf8(name); bool same=ls_string_equal(key,decoded); ls_string_release(decoded);
                if(same && match->capture[2*group]) { ls_regex_capture_text(out,match,group); break; }
                name+=strlen(name)+LRE_GROUP_NAME_TRAILER_LEN;
            }
            i=close;
        } else ls_string_builder_unit(out,'$');
    }
}
static LS_NATIVE_UNUSED ls_string ls_regex_replace(ls_string input, ls_native_object *owner, ls_string replacement) {
    ls_regex *regex=(ls_regex *)owner; bool global=(regex->bits&LRE_FLAG_GLOBAL)!=0;
    bool unicode=(regex->bits&(LRE_FLAG_UNICODE|LRE_FLAG_UNICODE_SETS))!=0;
    if(global) regex->last_index=0;
    ls_regex_context context={(uintptr_t)__builtin_frame_address(0),0,false,false};
    ls_string_builder out={0}; size_t emitted=0;
    for(;;) {
        ls_regex_match match={0}; bool found=ls_regex_exec(regex,input,&match,&context);
        if(!found) { free(match.capture); break; }
        size_t start=(size_t)((const uint16_t *)match.capture[0]-match.data);
        size_t end=(size_t)((const uint16_t *)match.capture[1]-match.data);
        ls_string_builder_text(&out,(ls_string){match.data+emitted,start-emitted,NULL});
        ls_regex_substitute(&out,regex,&match,input,replacement,start,end); emitted=end;
        free(match.capture);
        if(!global) break;
        if(start==end) {
            size_t next=end+1;
            if(unicode && end+1<input.length && is_hi_surrogate(input.data[end]) && is_lo_surrogate(input.data[end+1])) next++;
            regex->last_index=(double)next;
        }
    }
    if(ls_native_raised) { free(out.units); return (ls_string){0}; }
    if(input.length>emitted) ls_string_builder_text(&out,(ls_string){input.data+emitted,input.length-emitted,NULL});
    return ls_string_builder_finish(&out);
}
