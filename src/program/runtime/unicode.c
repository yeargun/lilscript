/* Default Unicode case conversion over UTF-16. Lone surrogates are preserved.
   Contextual final sigma uses the original string, not its converted prefix. */
static LS_NATIVE_UNUSED inline uint32_t ls_string_next_point(ls_string text, size_t *at) {
    uint32_t point=text.data[(*at)++];
    if(is_hi_surrogate(point) && *at<text.length && is_lo_surrogate(text.data[*at]))
        point=from_surrogate(point,text.data[(*at)++]);
    return point;
}
static LS_NATIVE_UNUSED inline void ls_string_builder_point(ls_string_builder *out, uint32_t point) {
    if(point>0xFFFF) {
        ls_string_builder_unit(out,(uint16_t)get_hi_surrogate(point));
        ls_string_builder_unit(out,(uint16_t)get_lo_surrogate(point));
    } else ls_string_builder_unit(out,(uint16_t)point);
}
static LS_NATIVE_UNUSED inline ls_string ls_string_case(ls_string text, bool upper) {
    ls_string_builder out={0}; bool preceding_cased=false;
    for(size_t at=0;at<text.length;) {
        uint32_t point=ls_string_next_point(text,&at),mapped[LRE_CC_RES_LEN_MAX];
        int length;
        bool final_sigma=!upper && point==0x03A3 && preceding_cased;
        if(final_sigma) {
            size_t next=at;
            while(next<text.length) {
                uint32_t after=ls_string_next_point(text,&next);
                if(!lre_is_case_ignorable(after)) { final_sigma=!lre_is_cased(after); break; }
            }
        }
        if(final_sigma) { mapped[0]=0x03C2; length=1; }
        else length=lre_case_conv(mapped,point,upper?0:1);
        for(int i=0;i<length;i++) ls_string_builder_point(&out,mapped[i]);
        if(!lre_is_case_ignorable(point)) preceding_cased=lre_is_cased(point)!=0;
    }
    return ls_string_builder_finish(&out);
}
