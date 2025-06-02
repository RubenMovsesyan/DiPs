import cv2
import numpy as np


# Maximum time to save the video as
MAX_DURATION = 20
INPUT_FILE_NAME = "test.avi"
OUTPUT_FILE_NAME = "output.avi"
SUB_SAMPLING_FACTOR = 10

# Get the video and necessary information
cap = cv2.VideoCapture(INPUT_FILE_NAME)
width = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH) + 0.5)
height = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT) + 0.5)
total_frames = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))

print("Width, Height:", width, height)
print("Total Frames:", total_frames)
print()

new_frames = total_frames / SUB_SAMPLING_FACTOR
new_fps = new_frames / MAX_DURATION

print("New Total Frames:", new_frames)
print("New FPS:", new_fps)


# Create the output writer
fourcc = cv2.VideoWriter.fourcc(*'MJPG')
out = cv2.VideoWriter(OUTPUT_FILE_NAME, fourcc, float(new_fps), (width, height))

index = 0

while cap.isOpened():
    ret, frame = cap.read()

    if ret == True:
        if index % SUB_SAMPLING_FACTOR == 0:
            out.write(frame)

        cv2.imshow("Frame", frame)
    else:
        break

    index += 1

cap.release()
out.release()
cv2.destroyAllWindows()
