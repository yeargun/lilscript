/* Strict JSON into the ordinary tagged collection ABI. Frames are heap-backed;
   source nesting does not consume the native call stack. All partial values
   have explicit owners and are released when parsing fails. */
static LS_NATIVE_UNUSED void ls_json_item_retain(const void *slot) { ls_value_retain(*(const ls_value *)slot); }
static LS_NATIVE_UNUSED void ls_json_item_drop(void *slot) { ls_value_clear(slot); }
static LS_NATIVE_UNUSED void ls_json_item_trace(const void *slot,ls_native_visit visit,void *context) { ls_value_trace(*(const ls_value *)slot,visit,context); }
static LS_NATIVE_UNUSED ls_value ls_json_item_read(ls_native_temporary **temps,const void *slot) { (void)temps; return *(const ls_value *)slot; }
static LS_NATIVE_UNUSED void ls_json_item_write(void *slot,ls_value value) { ls_value_copy(slot,value); }
static const ls_array_ops ls_json_array_ops={sizeof(ls_value),ls_json_item_retain,ls_json_item_drop,ls_json_item_trace,ls_json_item_read,ls_json_item_write};
typedef struct { ls_string source; size_t at; } ls_json_reader;
typedef struct { ls_value value; ls_string key; unsigned state; } ls_json_frame;
static LS_NATIVE_UNUSED void ls_json_syntax(void) {
    ls_native_raise_error("SyntaxError","Invalid JSON text");
}
static LS_NATIVE_UNUSED void ls_json_space(ls_json_reader *reader) {
    while(reader->at<reader->source.length) {
        uint16_t c=reader->source.data[reader->at];
        if(c!=9 && c!=10 && c!=13 && c!=32) break;
        reader->at++;
    }
}
static LS_NATIVE_UNUSED bool ls_json_take(ls_json_reader *reader,uint16_t c) {
    if(reader->at==reader->source.length || reader->source.data[reader->at]!=c) return false;
    reader->at++; return true;
}
static LS_NATIVE_UNUSED ls_string ls_json_read_string(ls_json_reader *reader) {
    ls_string_builder text={0};
    if(!ls_json_take(reader,'"')) { ls_json_syntax(); return (ls_string){0}; }
    while(reader->at<reader->source.length) {
        uint16_t c=reader->source.data[reader->at++];
        if(c=='"') return ls_string_builder_finish(&text);
        if(c<32) break;
        if(c=='\\') {
            if(reader->at==reader->source.length) break;
            c=reader->source.data[reader->at++];
            switch(c) {
            case '"': case '\\': case '/': break;
            case 'b': c=8; break;
            case 'f': c=12; break;
            case 'n': c=10; break;
            case 'r': c=13; break;
            case 't': c=9; break;
            case 'u': {
                if(reader->source.length-reader->at<4) goto invalid;
                uint16_t unit=0;
                for(size_t i=0;i<4;i++) {
                    int digit=from_hex(reader->source.data[reader->at++]);
                    if(digit<0) goto invalid;
                    unit=(uint16_t)((unit<<4)|digit);
                }
                c=unit; break;
            }
            default: goto invalid;
            }
        }
        ls_string_builder_unit(&text,c);
    }
invalid:
    free(text.units); ls_json_syntax(); return (ls_string){0};
}
static LS_NATIVE_UNUSED bool ls_json_digit(ls_json_reader *reader) {
    return reader->at<reader->source.length && reader->source.data[reader->at]>='0' && reader->source.data[reader->at]<='9';
}
static LS_NATIVE_UNUSED ls_value ls_json_read_number(ls_json_reader *reader) {
    size_t start=reader->at;
    ls_json_take(reader,'-');
    if(!ls_json_digit(reader)) { ls_json_syntax(); return (ls_value){0}; }
    if(!ls_json_take(reader,'0')) { do { reader->at++; } while(ls_json_digit(reader)); }
    if(ls_json_take(reader,'.')) {
        if(!ls_json_digit(reader)) { ls_json_syntax(); return (ls_value){0}; }
        do { reader->at++; } while(ls_json_digit(reader));
    }
    if(ls_json_take(reader,'e') || ls_json_take(reader,'E')) {
        if(!ls_json_take(reader,'+')) ls_json_take(reader,'-');
        if(!ls_json_digit(reader)) { ls_json_syntax(); return (ls_value){0}; }
        do { reader->at++; } while(ls_json_digit(reader));
    }
    size_t length=reader->at-start;
    char *text=malloc(length+1);
    if(!text) ls_native_resource_failure();
    for(size_t i=0;i<length;i++) text[i]=(char)reader->source.data[start+i];
    text[length]=0;
    double number=ls_decimal_parse(text,length); free(text);
    /* Numbers are one source domain. An exact int32 gets the existing integer
       representation, while negative zero and every other binary64 stay float. */
    if(number>=INT32_MIN && number<=INT32_MAX && number==trunc(number) && !(number==0 && signbit(number)))
        return ls_value_int((int32_t)number);
    return ls_value_float(number);
}
static LS_NATIVE_UNUSED bool ls_json_word(ls_json_reader *reader,const char *word) {
    for(size_t i=0;word[i];i++) if(!ls_json_take(reader,(uint16_t)word[i])) return false;
    return true;
}
static LS_NATIVE_UNUSED ls_value ls_json_parse(ls_string source) {
    ls_json_reader reader={source,0};
    ls_json_frame *frames=NULL; size_t depth=0,capacity=0;
    ls_value current={0}; bool need_value=true;
    for(;;) {
        ls_json_space(&reader);
        if(need_value) {
            if(reader.at==source.length) goto invalid;
            uint16_t c=source.data[reader.at];
            if(c=='{' || c=='[') {
                if(depth==capacity) {
                    size_t next=capacity?capacity*2:16;
                    if(next<capacity || next>SIZE_MAX/sizeof *frames) ls_native_resource_failure();
                    ls_json_frame *grown=realloc(frames,next*sizeof *frames);
                    if(!grown) ls_native_resource_failure();
                    frames=grown; capacity=next;
                }
                reader.at++;
                current=c=='{'?ls_value_object(ls_record_new()):ls_value_array((ls_native_object *)ls_array_new(&ls_json_array_ops,0));
                frames[depth++]=(ls_json_frame){current,{0},0}; current=(ls_value){0};
                need_value=false; continue;
            }
            if(c=='"') current=ls_value_string(ls_json_read_string(&reader));
            else if(c=='t') { if(!ls_json_word(&reader,"true")) goto invalid; current=ls_value_bool(true); }
            else if(c=='f') { if(!ls_json_word(&reader,"false")) goto invalid; current=ls_value_bool(false); }
            else if(c=='n') { if(!ls_json_word(&reader,"null")) goto invalid; current=(ls_value){0}; }
            else if(c=='-' || (c>='0' && c<='9')) current=ls_json_read_number(&reader);
            else goto invalid;
            if(ls_native_raised) goto failed;
        } else {
            ls_json_frame *frame=&frames[depth-1];
            bool object=frame->value.tag==LS_OBJECT; uint16_t close=object?'}':']';
            if(frame->state<2) {
                if(frame->state==0 && ls_json_take(&reader,close)) {
                    current=frame->value; depth--;
                } else {
                    if(object) {
                        frame->key=ls_json_read_string(&reader);
                        if(ls_native_raised) goto failed;
                        ls_json_space(&reader);
                        if(!ls_json_take(&reader,':')) goto invalid;
                    }
                    frame->state=2; need_value=true; continue;
                }
            } else {
                if(ls_json_take(&reader,close)) { current=frame->value; depth--; }
                else if(ls_json_take(&reader,',')) { frame->state=1; continue; }
                else goto invalid;
            }
        }
        if(!depth) {
            ls_json_space(&reader);
            if(reader.at!=source.length) goto invalid;
            free(frames); return current;
        }
        ls_json_frame *parent=&frames[depth-1];
        if(parent->value.tag==LS_OBJECT) {
            ls_record_set(parent->value.as.o,parent->key,current);
            ls_string_release(parent->key); parent->key=(ls_string){0};
        } else {
            /* The fresh slot takes this owner without a retain/release pair. */
            *(ls_value *)ls_array_append_slot((ls_native_array *)parent->value.as.o)=current;
            current=(ls_value){0};
        }
        ls_value_release(current); current=(ls_value){0};
        parent->state=3; need_value=false;
    }
invalid:
    ls_json_syntax();
failed:
    ls_value_release(current);
    for(size_t i=0;i<depth;i++) { ls_value_release(frames[i].value); ls_string_release(frames[i].key); }
    free(frames); return (ls_value){0};
}
