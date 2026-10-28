// Visual Pinball Infinite (GPL-3.0-or-later): the bridge between the player and FFmpeg for
// PinUP packs' media. Ours: VPX's own media player and FFmpeg loader carry no GPL mark
// and aren't ported. It opens a file, decodes its video to RGBA pictures and its sound to
// stereo floats at the mixer's rate, one piece at a time, in the file's order; the player's
// threads do the timing. And it writes a video — a table's preview, its pictures handed over
// one at a time — as H.264 through Windows' own encoder (Media Foundation's, `h264_mf`) in an
// MP4, with the table's sound beside them, if wanted, as AAC (FFmpeg's own encoder). FFmpeg's
// libraries are loaded the first time a pack plays, from a folder the player names, so the
// player starts and plays tables without them.
//
// Written in C++ only to type-check the table of FFmpeg's functions against its own headers
// (decltype); no C++ library is used. FFmpeg (LGPL-2.1-or-later; GPL in builds with its GPL
// parts) is called through that table, never linked.

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavutil/channel_layout.h>
#include <libswresample/swresample.h>
#include <libswscale/swscale.h>
}

// ---------------------------------------------------------------- FFmpeg's functions

#define FN(name) decltype(&::name) name
static struct {
   FN(avformat_open_input);
   FN(avformat_find_stream_info);
   FN(av_find_best_stream);
   FN(av_read_frame);
   FN(av_seek_frame);
   FN(avformat_close_input);
   FN(avcodec_alloc_context3);
   FN(avcodec_parameters_to_context);
   FN(avcodec_open2);
   FN(avcodec_send_packet);
   FN(avcodec_receive_frame);
   FN(avcodec_flush_buffers);
   FN(avcodec_free_context);
   FN(av_packet_alloc);
   FN(av_packet_unref);
   FN(av_packet_free);
   FN(av_frame_alloc);
   FN(av_frame_free);
   FN(av_log_set_level);
   FN(av_channel_layout_default);
   FN(av_channel_layout_uninit);
   FN(sws_getContext);
   FN(sws_scale);
   FN(sws_freeContext);
   FN(swr_alloc_set_opts2);
   FN(swr_init);
   FN(swr_convert);
   FN(swr_get_out_samples);
   FN(swr_free);
} av;

// Its functions for writing a video, apart from the reading ones: a build without them still
// plays packs.
static struct {
   FN(avformat_alloc_output_context2);
   FN(avformat_new_stream);
   FN(avio_open);
   FN(avio_closep);
   FN(avformat_write_header);
   FN(av_interleaved_write_frame);
   FN(av_write_trailer);
   FN(avformat_free_context);
   FN(avcodec_find_encoder_by_name);
   FN(avcodec_parameters_from_context);
   FN(avcodec_send_frame);
   FN(avcodec_receive_packet);
   FN(av_packet_rescale_ts);
   FN(av_frame_get_buffer);
   FN(av_frame_make_writable);
   FN(av_dict_set);
   FN(av_dict_free);
} enc;
static bool enc_ok = false;
#undef FN

// One of FFmpeg's libraries from `dir`, by either name builds give it ("avcodec64-62.dll" as
// VPX's build names them, "avcodec-62.dll" as FFmpeg's own do), its own folder searched for
// the libraries it needs in turn.
static HMODULE load_library(const wchar_t *dir, const wchar_t *name, int major)
{
   wchar_t path[MAX_PATH * 2];
   const wchar_t *patterns[] = { L"%ls\\%ls64-%d.dll", L"%ls\\%ls-%d.dll" };
   for (const wchar_t *pattern : patterns)
   {
      _snwprintf_s(path, _countof(path), _TRUNCATE, pattern, dir, name, major);
      HMODULE m = LoadLibraryExW(path, nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
      if (m)
         return m;
   }
   return nullptr;
}

template <typename T> static bool find(HMODULE m, T &slot, const char *name)
{
   slot = m ? reinterpret_cast<T>(reinterpret_cast<void *>(GetProcAddress(m, name))) : nullptr;
   return slot != nullptr;
}

// Loads FFmpeg's libraries from `dir`: 1 when every function is there. Called once.
extern "C" int vpi_av_load(const wchar_t *dir)
{
   HMODULE util = load_library(dir, L"avutil", LIBAVUTIL_VERSION_MAJOR);
   HMODULE resample = load_library(dir, L"swresample", LIBSWRESAMPLE_VERSION_MAJOR);
   HMODULE scale = load_library(dir, L"swscale", LIBSWSCALE_VERSION_MAJOR);
   HMODULE codec = load_library(dir, L"avcodec", LIBAVCODEC_VERSION_MAJOR);
   HMODULE format = load_library(dir, L"avformat", LIBAVFORMAT_VERSION_MAJOR);
   bool ok = true;
#define GET(lib, name) ok &= find(lib, av.name, #name)
   GET(format, avformat_open_input);
   GET(format, avformat_find_stream_info);
   GET(format, av_find_best_stream);
   GET(format, av_read_frame);
   GET(format, av_seek_frame);
   GET(format, avformat_close_input);
   GET(codec, avcodec_alloc_context3);
   GET(codec, avcodec_parameters_to_context);
   GET(codec, avcodec_open2);
   GET(codec, avcodec_send_packet);
   GET(codec, avcodec_receive_frame);
   GET(codec, avcodec_flush_buffers);
   GET(codec, avcodec_free_context);
   GET(codec, av_packet_alloc);
   GET(codec, av_packet_unref);
   GET(codec, av_packet_free);
   GET(util, av_frame_alloc);
   GET(util, av_frame_free);
   GET(util, av_log_set_level);
   GET(util, av_channel_layout_default);
   GET(util, av_channel_layout_uninit);
   GET(scale, sws_getContext);
   GET(scale, sws_scale);
   GET(scale, sws_freeContext);
   GET(resample, swr_alloc_set_opts2);
   GET(resample, swr_init);
   GET(resample, swr_convert);
   GET(resample, swr_get_out_samples);
   GET(resample, swr_free);
#undef GET
   bool writing = true;
#define GET(lib, name) writing &= find(lib, enc.name, #name)
   GET(format, avformat_alloc_output_context2);
   GET(format, avformat_new_stream);
   GET(format, avio_open);
   GET(format, avio_closep);
   GET(format, avformat_write_header);
   GET(format, av_interleaved_write_frame);
   GET(format, av_write_trailer);
   GET(format, avformat_free_context);
   GET(codec, avcodec_find_encoder_by_name);
   GET(codec, avcodec_parameters_from_context);
   GET(codec, avcodec_send_frame);
   GET(codec, avcodec_receive_packet);
   GET(codec, av_packet_rescale_ts);
   GET(util, av_frame_get_buffer);
   GET(util, av_frame_make_writable);
   GET(util, av_dict_set);
   GET(util, av_dict_free);
#undef GET
   enc_ok = writing;
   // FFmpeg's own messages, for finding out why something won't play or write: `VPI_AV_LOG=1`
   char log[8];
   size_t log_len = 0;
   if (ok)
      av.av_log_set_level(getenv_s(&log_len, log, sizeof log, "VPI_AV_LOG") == 0 && log_len > 1 ? AV_LOG_VERBOSE : AV_LOG_QUIET);
   return ok ? 1 : 0;
}

// ---------------------------------------------------------------- a file

struct VpiAvInfo
{
   int32_t has_video;
   int32_t has_audio;
   int32_t width;
   int32_t height;
   double fps;
   int64_t duration_ms; // -1: not known
};

// One piece of what the file holds: a picture (kind 1) or some sound (kind 2). Its data
// stays the bridge's until the next call.
struct VpiAvItem
{
   int32_t kind;
   int64_t pts_ms; // from the stream's start
   int32_t width;
   int32_t height;
   const uint8_t *rgba; // width * height * 4 bytes, rows top first
   int32_t frames;      // stereo frames
   const float *pcm;    // frames * 2, interleaved
};

struct Stream
{
   int index = -1;
   AVCodecContext *ctx = nullptr;
   double ms_per_tick = 0.0; // the stream's time base, in ms
   int64_t start = 0;        // its first tick
   bool pending = false;     // the decoder may have output waiting
};

struct VpiAvMedia
{
   AVFormatContext *fmt = nullptr;
   Stream video, audio;
   AVPacket *packet = nullptr;
   AVFrame *frame = nullptr;
   bool ended = false; // the file is read to its end and the decoders flushed
   SwsContext *sws = nullptr;
   int sws_w = 0, sws_h = 0, sws_format = -1;
   uint8_t *rgba = nullptr;
   size_t rgba_size = 0;
   SwrContext *swr = nullptr;
   int out_rate = 48000;
   float *pcm = nullptr;
   int pcm_frames = 0;
};

// Both streams, for going through them (an array: no C++ library is used).
struct Streams
{
   Stream *s[2];
   Stream **begin() { return s; }
   Stream **end() { return s + 2; }
};
static Streams streams(VpiAvMedia *m)
{
   return Streams { { &m->video, &m->audio } };
}

static void close_media(VpiAvMedia *m)
{
   if (!m)
      return;
   if (m->video.ctx)
      av.avcodec_free_context(&m->video.ctx);
   if (m->audio.ctx)
      av.avcodec_free_context(&m->audio.ctx);
   if (m->fmt)
      av.avformat_close_input(&m->fmt);
   if (m->packet)
      av.av_packet_free(&m->packet);
   if (m->frame)
      av.av_frame_free(&m->frame);
   if (m->sws)
      av.sws_freeContext(m->sws);
   if (m->swr)
      av.swr_free(&m->swr);
   free(m->rgba);
   free(m->pcm);
   delete m;
}

static bool open_stream(VpiAvMedia *m, AVMediaType type, Stream &s)
{
   const AVCodec *codec = nullptr;
   const int index = av.av_find_best_stream(m->fmt, type, -1, -1, &codec, 0);
   if (index < 0 || !codec)
      return false;
   AVStream *st = m->fmt->streams[index];
   // a sound file's cover picture isn't video
   if (type == AVMEDIA_TYPE_VIDEO && (st->disposition & AV_DISPOSITION_ATTACHED_PIC))
      return false;
   AVCodecContext *ctx = av.avcodec_alloc_context3(codec);
   if (!ctx)
      return false;
   if (av.avcodec_parameters_to_context(ctx, st->codecpar) < 0 || av.avcodec_open2(ctx, codec, nullptr) < 0)
   {
      av.avcodec_free_context(&ctx);
      return false;
   }
   s.index = index;
   s.ctx = ctx;
   s.ms_per_tick = 1000.0 * st->time_base.num / (st->time_base.den ? st->time_base.den : 1);
   s.start = st->start_time != AV_NOPTS_VALUE ? st->start_time : 0;
   return true;
}

// Opens a file (its path in UTF-8, as FFmpeg takes it), for its pictures, its sound, or both,
// the sound made stereo at `out_rate`. Null when it can't be read or has neither.
extern "C" VpiAvMedia *vpi_av_open(const char *path, int32_t want_video, int32_t want_audio, int32_t out_rate, VpiAvInfo *info)
{
   if (!av.avformat_open_input)
      return nullptr;
   VpiAvMedia *m = new VpiAvMedia();
   m->out_rate = out_rate > 0 ? out_rate : 48000;
   if (av.avformat_open_input(&m->fmt, path, nullptr, nullptr) < 0 || av.avformat_find_stream_info(m->fmt, nullptr) < 0)
   {
      close_media(m);
      return nullptr;
   }
   const bool video = want_video && open_stream(m, AVMEDIA_TYPE_VIDEO, m->video);
   const bool audio = want_audio && open_stream(m, AVMEDIA_TYPE_AUDIO, m->audio);
   m->packet = av.av_packet_alloc();
   m->frame = av.av_frame_alloc();
   if ((!video && !audio) || !m->packet || !m->frame)
   {
      close_media(m);
      return nullptr;
   }
   if (info)
   {
      memset(info, 0, sizeof(*info));
      info->has_video = video;
      info->has_audio = audio;
      if (video)
      {
         AVStream *st = m->fmt->streams[m->video.index];
         info->width = m->video.ctx->width;
         info->height = m->video.ctx->height;
         info->fps = st->avg_frame_rate.den ? (double)st->avg_frame_rate.num / st->avg_frame_rate.den : 0.0;
      }
      info->duration_ms = m->fmt->duration != AV_NOPTS_VALUE ? m->fmt->duration / (AV_TIME_BASE / 1000) : -1;
   }
   return m;
}

static int64_t pts_ms(const Stream &s, const AVFrame *f)
{
   int64_t t = f->best_effort_timestamp != AV_NOPTS_VALUE ? f->best_effort_timestamp : f->pts;
   if (t == AV_NOPTS_VALUE)
      return 0;
   return (int64_t)((t - s.start) * s.ms_per_tick);
}

// The decoded picture as RGBA.
static bool picture(VpiAvMedia *m, VpiAvItem *item)
{
   AVFrame *f = m->frame;
   const int w = f->width, h = f->height;
   if (w <= 0 || h <= 0)
      return false;
   if (!m->sws || w != m->sws_w || h != m->sws_h || f->format != m->sws_format)
   {
      if (m->sws)
         av.sws_freeContext(m->sws);
      m->sws = av.sws_getContext(w, h, (AVPixelFormat)f->format, w, h, AV_PIX_FMT_RGBA, SWS_BILINEAR, nullptr, nullptr, nullptr);
      m->sws_w = w;
      m->sws_h = h;
      m->sws_format = f->format;
      if (!m->sws)
         return false;
   }
   const size_t size = (size_t)w * h * 4;
   if (size > m->rgba_size)
   {
      uint8_t *bigger = (uint8_t *)realloc(m->rgba, size);
      if (!bigger)
         return false;
      m->rgba = bigger;
      m->rgba_size = size;
   }
   uint8_t *dst[4] = { m->rgba, nullptr, nullptr, nullptr };
   const int dst_stride[4] = { w * 4, 0, 0, 0 };
   av.sws_scale(m->sws, f->data, f->linesize, 0, h, dst, dst_stride);
   item->kind = 1;
   item->pts_ms = pts_ms(m->video, f);
   item->width = w;
   item->height = h;
   item->rgba = m->rgba;
   return true;
}

// The decoded sound as stereo floats at the output rate.
static bool sound(VpiAvMedia *m, VpiAvItem *item)
{
   AVFrame *f = m->frame;
   if (!m->swr)
   {
      AVChannelLayout stereo;
      av.av_channel_layout_default(&stereo, 2);
      const int r = av.swr_alloc_set_opts2(&m->swr, &stereo, AV_SAMPLE_FMT_FLT, m->out_rate, &f->ch_layout, (AVSampleFormat)f->format, f->sample_rate, 0, nullptr);
      av.av_channel_layout_uninit(&stereo);
      if (r < 0 || av.swr_init(m->swr) < 0)
      {
         if (m->swr)
            av.swr_free(&m->swr);
         return false;
      }
   }
   const int most = av.swr_get_out_samples(m->swr, f->nb_samples);
   if (most <= 0)
      return false;
   if (most > m->pcm_frames)
   {
      float *bigger = (float *)realloc(m->pcm, (size_t)most * 2 * sizeof(float));
      if (!bigger)
         return false;
      m->pcm = bigger;
      m->pcm_frames = most;
   }
   uint8_t *out[1] = { (uint8_t *)m->pcm };
   const int n = av.swr_convert(m->swr, out, most, (const uint8_t *const *)f->extended_data, f->nb_samples);
   if (n <= 0)
      return false;
   item->kind = 2;
   item->pts_ms = pts_ms(m->audio, f);
   item->frames = n;
   item->pcm = m->pcm;
   return true;
}

// The next picture or piece of sound, in the file's order: 1 when there is one, 0 at the end.
extern "C" int32_t vpi_av_next(VpiAvMedia *m, VpiAvItem *item)
{
   if (!m || !item)
      return 0;
   memset(item, 0, sizeof(*item));
   for (;;)
   {
      // what the decoders have ready
      for (Stream *s : streams(m))
      {
         while (s->ctx && s->pending)
         {
            const int r = av.avcodec_receive_frame(s->ctx, m->frame);
            if (r < 0)
            {
               s->pending = false; // wants more, or done
               break;
            }
            const bool got = s == &m->video ? picture(m, item) : sound(m, item);
            if (got)
               return 1;
         }
      }
      if (m->ended)
         return 0;
      const int r = av.av_read_frame(m->fmt, m->packet);
      if (r < 0)
      {
         // the end: what the decoders hold back comes out
         for (Stream *s : streams(m))
         {
            if (s->ctx)
            {
               av.avcodec_send_packet(s->ctx, nullptr);
               s->pending = true;
            }
         }
         m->ended = true;
         continue;
      }
      for (Stream *s : streams(m))
      {
         if (s->ctx && m->packet->stream_index == s->index && av.avcodec_send_packet(s->ctx, m->packet) >= 0)
            s->pending = true;
      }
      av.av_packet_unref(m->packet);
   }
}

// Back to the start, for a file that loops: 1 when it could.
extern "C" int32_t vpi_av_rewind(VpiAvMedia *m)
{
   if (!m || av.av_seek_frame(m->fmt, -1, m->fmt->start_time != AV_NOPTS_VALUE ? m->fmt->start_time : 0, AVSEEK_FLAG_BACKWARD) < 0)
      return 0;
   for (Stream *s : streams(m))
   {
      if (s->ctx)
         av.avcodec_flush_buffers(s->ctx);
      s->pending = false;
   }
   m->ended = false;
   return 1;
}

extern "C" void vpi_av_close(VpiAvMedia *m)
{
   close_media(m);
}

// ---------------------------------------------------------------- writing a video

// One stream being written: its encoder, its stream in the file, the frame filled for it.
struct VpiAvTrack
{
   AVCodecContext *ctx = nullptr;
   AVStream *stream = nullptr;
   AVFrame *frame = nullptr;
   int64_t next_pts = 0;
};

struct VpiAvWriter
{
   AVFormatContext *fmt = nullptr;
   AVPacket *packet = nullptr;
   VpiAvTrack video, sound;
   SwsContext *sws = nullptr;
   int width = 0, height = 0;
   // the sound's frames: their whole size, and the samples in the one being filled
   int sound_frame = 0, pending = 0;
};

static void free_track(VpiAvTrack &t)
{
   if (t.ctx)
      av.avcodec_free_context(&t.ctx);
   if (t.frame)
      av.av_frame_free(&t.frame);
}

static void free_writer(VpiAvWriter *w)
{
   if (!w)
      return;
   if (w->fmt)
   {
      if (w->fmt->pb && !(w->fmt->oformat->flags & AVFMT_NOFILE))
         enc.avio_closep(&w->fmt->pb);
      enc.avformat_free_context(w->fmt);
   }
   free_track(w->video);
   free_track(w->sound);
   if (w->packet)
      av.av_packet_free(&w->packet);
   if (w->sws)
      av.sws_freeContext(w->sws);
   delete w;
}

// What a track's encoder has ready, into the file: false when it couldn't be written.
static bool drain(VpiAvWriter *w, VpiAvTrack &t)
{
   for (;;)
   {
      const int r = enc.avcodec_receive_packet(t.ctx, w->packet);
      if (r == AVERROR(EAGAIN) || r == AVERROR_EOF)
         return true;
      if (r < 0)
         return false;
      enc.av_packet_rescale_ts(w->packet, t.ctx->time_base, t.stream->time_base);
      w->packet->stream_index = t.stream->index;
      // the file takes the packet's data, the two tracks' packets interleaved by their times
      if (enc.av_interleaved_write_frame(w->fmt, w->packet) < 0)
         return false;
   }
}

static VpiAvWriter *writer_failed(VpiAvWriter *w, const char *why, char *error, int32_t error_len)
{
   if (error && error_len > 0)
      strncpy_s(error, (size_t)error_len, why, _TRUNCATE);
   free_writer(w);
   return nullptr;
}

// A video to write (its path in UTF-8): `width` × `height` pictures, `fps` a second, as H.264
// through Windows' own encoder in an MP4 that plays before it's all read (its index first); with
// `sound_rate` above nought, its sound beside them — stereo floats at that rate, as AAC (FFmpeg's
// own encoder). Null when it can't be made, `error` saying why.
extern "C" VpiAvWriter *vpi_av_writer_open(const char *path, int32_t width, int32_t height, int32_t fps, int32_t sound_rate, char *error, int32_t error_len)
{
   if (!enc_ok)
      return writer_failed(nullptr, "FFmpeg's functions for writing a video aren't there", error, error_len);
   const AVCodec *codec = enc.avcodec_find_encoder_by_name("h264_mf");
   if (!codec)
      return writer_failed(nullptr, "FFmpeg has no H.264 encoder through Windows (h264_mf)", error, error_len);
   const AVCodec *aac = sound_rate > 0 ? enc.avcodec_find_encoder_by_name("aac") : nullptr;
   if (sound_rate > 0 && !aac)
      return writer_failed(nullptr, "FFmpeg has no AAC encoder", error, error_len);
   VpiAvWriter *w = new VpiAvWriter();
   w->width = width;
   w->height = height;
   if (enc.avformat_alloc_output_context2(&w->fmt, nullptr, "mp4", path) < 0 || !w->fmt)
      return writer_failed(w, "an MP4 couldn't be made", error, error_len);
   // the pictures held until the sound beside them comes, however late (it can come whole, at
   // the end), so the two tracks lie interleaved in the file
   w->fmt->max_interleave_delta = 0;
   VpiAvTrack &v = w->video;
   v.ctx = av.avcodec_alloc_context3(codec);
   if (!v.ctx)
      return writer_failed(w, "the encoder couldn't be made", error, error_len);
   v.ctx->width = width;
   v.ctx->height = height;
   v.ctx->time_base = AVRational { 1, fps };
   v.ctx->framerate = AVRational { fps, 1 };
   // Media Foundation's encoder takes NV12
   v.ctx->pix_fmt = AV_PIX_FMT_NV12;
   // about 8 Mbit a second for 1080 × 1920 at 25 a second: a sharp preview, a few MB long
   v.ctx->bit_rate = (int64_t)width * height * fps / 6;
   v.ctx->gop_size = fps * 2;
   if (w->fmt->oformat->flags & AVFMT_GLOBALHEADER)
      v.ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
   if (av.avcodec_open2(v.ctx, codec, nullptr) < 0)
      return writer_failed(w, "Windows' H.264 encoder wouldn't open", error, error_len);
   v.stream = enc.avformat_new_stream(w->fmt, nullptr);
   if (!v.stream || enc.avcodec_parameters_from_context(v.stream->codecpar, v.ctx) < 0)
      return writer_failed(w, "the video's stream couldn't be made", error, error_len);
   v.stream->time_base = v.ctx->time_base;
   if (aac)
   {
      VpiAvTrack &s = w->sound;
      s.ctx = av.avcodec_alloc_context3(aac);
      if (!s.ctx)
         return writer_failed(w, "the sound's encoder couldn't be made", error, error_len);
      // FFmpeg's AAC encoder takes each channel apart, as floats
      s.ctx->sample_fmt = AV_SAMPLE_FMT_FLTP;
      s.ctx->sample_rate = sound_rate;
      av.av_channel_layout_default(&s.ctx->ch_layout, 2);
      s.ctx->bit_rate = 160000;
      s.ctx->time_base = AVRational { 1, sound_rate };
      if (w->fmt->oformat->flags & AVFMT_GLOBALHEADER)
         s.ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
      if (av.avcodec_open2(s.ctx, aac, nullptr) < 0)
         return writer_failed(w, "the AAC encoder wouldn't open", error, error_len);
      s.stream = enc.avformat_new_stream(w->fmt, nullptr);
      if (!s.stream || enc.avcodec_parameters_from_context(s.stream->codecpar, s.ctx) < 0)
         return writer_failed(w, "the sound's stream couldn't be made", error, error_len);
      s.stream->time_base = s.ctx->time_base;
   }
   if (enc.avio_open(&w->fmt->pb, path, AVIO_FLAG_WRITE) < 0)
      return writer_failed(w, "the file couldn't be written", error, error_len);
   AVDictionary *options = nullptr;
   enc.av_dict_set(&options, "movflags", "+faststart", 0);
   const int header = enc.avformat_write_header(w->fmt, &options);
   enc.av_dict_free(&options);
   if (header < 0)
      return writer_failed(w, "the MP4's start couldn't be written", error, error_len);
   w->packet = av.av_packet_alloc();
   v.frame = av.av_frame_alloc();
   if (!w->packet || !v.frame)
      return writer_failed(w, "out of memory", error, error_len);
   v.frame->format = v.ctx->pix_fmt;
   v.frame->width = width;
   v.frame->height = height;
   if (enc.av_frame_get_buffer(v.frame, 0) < 0)
      return writer_failed(w, "out of memory", error, error_len);
   if (aac)
   {
      VpiAvTrack &s = w->sound;
      s.frame = av.av_frame_alloc();
      if (!s.frame)
         return writer_failed(w, "out of memory", error, error_len);
      s.frame->format = s.ctx->sample_fmt;
      s.frame->sample_rate = sound_rate;
      s.frame->nb_samples = s.ctx->frame_size > 0 ? s.ctx->frame_size : 1024;
      av.av_channel_layout_default(&s.frame->ch_layout, 2);
      if (enc.av_frame_get_buffer(s.frame, 0) < 0)
         return writer_failed(w, "out of memory", error, error_len);
      // the frame's whole size, as `nb_samples` shrinks for a last, short one
      w->sound_frame = s.frame->nb_samples;
   }
   w->sws = av.sws_getContext(width, height, AV_PIX_FMT_RGBA, width, height, v.ctx->pix_fmt, SWS_BILINEAR, nullptr, nullptr, nullptr);
   if (!w->sws)
      return writer_failed(w, "the pictures' colours couldn't be converted", error, error_len);
   return w;
}

// One picture: width × height × 4 bytes of RGBA, rows top first. 1 when it's written.
extern "C" int32_t vpi_av_writer_frame(VpiAvWriter *w, const uint8_t *rgba)
{
   if (!w || enc.av_frame_make_writable(w->video.frame) < 0)
      return 0;
   const uint8_t *const source[1] = { rgba };
   const int stride[1] = { w->width * 4 };
   av.sws_scale(w->sws, source, stride, 0, w->height, w->video.frame->data, w->video.frame->linesize);
   w->video.frame->pts = w->video.next_pts++;
   if (enc.avcodec_send_frame(w->video.ctx, w->video.frame) < 0)
      return 0;
   return drain(w, w->video) ? 1 : 0;
}

// The samples in the sound's frame so far, to its encoder, and what it has ready into the file.
static bool send_sound(VpiAvWriter *w)
{
   VpiAvTrack &s = w->sound;
   s.frame->nb_samples = w->pending;
   s.frame->pts = s.next_pts;
   s.next_pts += w->pending;
   const bool ok = enc.avcodec_send_frame(s.ctx, s.frame) >= 0 && drain(w, s);
   s.frame->nb_samples = w->sound_frame;
   w->pending = 0;
   return ok;
}

// `frames` stereo frames of floats (left, right, …) for the video's sound, into its frames, each
// full one encoded. 1 when they're written; 0 for a video made without sound.
extern "C" int32_t vpi_av_writer_sound(VpiAvWriter *w, const float *samples, int32_t frames)
{
   if (!w || !w->sound.ctx)
      return 0;
   for (int32_t i = 0; i < frames; i++)
   {
      if (w->pending == 0 && enc.av_frame_make_writable(w->sound.frame) < 0)
         return 0;
      ((float *)w->sound.frame->data[0])[w->pending] = samples[2 * i];
      ((float *)w->sound.frame->data[1])[w->pending] = samples[2 * i + 1];
      if (++w->pending == w->sound_frame && !send_sound(w))
         return 0;
   }
   return 1;
}

// The video ended: its sound's last, short frame, what the encoders held back, then the file's
// end. 1 when all of it was written; the writer is freed either way.
extern "C" int32_t vpi_av_writer_close(VpiAvWriter *w)
{
   if (!w)
      return 0;
   bool ok = enc.avcodec_send_frame(w->video.ctx, nullptr) >= 0 && drain(w, w->video);
   if (w->sound.ctx)
   {
      ok = (w->pending == 0 || send_sound(w)) && ok;
      ok = enc.avcodec_send_frame(w->sound.ctx, nullptr) >= 0 && drain(w, w->sound) && ok;
   }
   ok = enc.av_write_trailer(w->fmt) >= 0 && ok;
   free_writer(w);
   return ok ? 1 : 0;
}
