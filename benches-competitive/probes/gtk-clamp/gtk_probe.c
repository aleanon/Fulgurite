/* Diagnostic ONLY — no timing. Replicates harnesses/gtk/frame.c's tree and
   prints what gtk_widget_measure() actually reports for the box, so the
   "GtkBox clamps to the window, ~63 rows at any N" caveat in
   docs/results-competitive.md can be checked rather than inherited. */
#include <gtk/gtk.h>
#include <stdio.h>
static int N = 3000;
static void pump(void){ while (g_main_context_pending(NULL)) g_main_context_iteration(NULL, FALSE); }
static void on_activate(GtkApplication *app, gpointer _u){ (void)_u;
    GtkWidget *win = gtk_application_window_new(app);
    gtk_window_set_default_size(GTK_WINDOW(win), 400, 800);
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    for (int i=0;i<N;i++){ char b[32]; snprintf(b,sizeof b,"row %d",i);
        gtk_box_append(GTK_BOX(box), gtk_label_new(b)); }
    GtkWidget *sc = gtk_scrolled_window_new();
    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(sc), box);
    gtk_window_set_child(GTK_WINDOW(win), sc);
    gtk_widget_set_visible(win, TRUE);
    for (int i=0;i<2000 && gtk_widget_get_width(sc)==0;i++){ pump(); g_usleep(1000); }
    int m,n2,a,b2;
    gtk_widget_measure(box, GTK_ORIENTATION_VERTICAL, 400, &m,&n2,&a,&b2);
    int one_m,one_n; GtkWidget *probe = gtk_label_new("row 0");
    gtk_widget_measure(probe, GTK_ORIENTATION_VERTICAL, 400, &one_m,&one_n,&a,&b2);
    printf("N=%d  measure(box).min=%d measure(box).nat=%d  label.min=%d  "
           "box_alloc_h=%d sc_alloc=%dx%d  rows_implied_by_min=%.1f\n",
           N, m, n2, one_m, gtk_widget_get_height(box),
           gtk_widget_get_width(sc), gtk_widget_get_height(sc),
           one_m ? (double)m/one_m : -1.0);
    g_application_quit(G_APPLICATION(app));
}
int main(int argc, char**argv){ if(argc>1) N=atoi(argv[1]);
    GtkApplication *app = gtk_application_new("dev.lumenstatic.probe", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(app,"activate",G_CALLBACK(on_activate),NULL);
    int r = g_application_run(G_APPLICATION(app),1,argv); g_object_unref(app); return r; }
