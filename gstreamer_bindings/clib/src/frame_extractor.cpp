#include "../include/frame_extractor.h"
#include <stdio.h>

void initialize_frame_extractor(int argc, char* argv[]) {
    const gchar* nano_str;
    guint major, minor, micro, nano;

    // Initialize GStreamer
    gst_init(&argc, &argv);

    gst_version(&major, &minor, &micro, &nano);    

    if (nano == 1) {
        nano_str = "(CVS)";
    } else if (nano == 2) {
        nano_str = "(Prerelease)";
    } else {
        nano_str = "";
    }

    printf("This program is linked against GStreamer %d.%d.%d %s\n", 
           major,
           minor,
           micro,
           nano_str
    );
}

GstElement* create_video_frame_decoder_pipeline() {
    
}
