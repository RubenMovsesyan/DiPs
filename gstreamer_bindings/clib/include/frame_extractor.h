#include <gst/gst.h>

void initialize_frame_extractor(int argc, char *argv[]);

// Returns the Pipeline
GstElement* create_video_frame_decoder_pipeline();
