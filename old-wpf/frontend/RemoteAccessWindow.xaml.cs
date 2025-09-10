using ESP32_Manager.backend;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Text;
using System.Text.Json.Nodes;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Threading;
using System.Windows.Media;

namespace ESP32_Manager.frontend
{
    public partial class RemoteAccessWindow : Window
    {
        private RemoteAccess remoteAccess;

        // ScrollViewer-Referenzen für beide TextBoxen
        private ScrollViewer udpScrollViewer;
        private ScrollViewer tcpScrollViewer;

        // Flags, um zu verfolgen, ob der Benutzer manuell nicht am Ende ist
        private bool udpUserNotAtEnd = false;
        private bool tcpUserNotAtEnd = false;

        public RemoteAccessWindow(string projectName, IPAddress ipAddress, ushort udpPort, ushort tcpPort)
        {
            InitializeComponent();
            Title += " - " + projectName;

            // ScrollViewer für TextBoxes finden
            udpScrollViewer = FindScrollViewer(udpMonitorTextField);
            tcpScrollViewer = FindScrollViewer(tcpMonitorTextField);

            // Ereignisse für manuelle Scrollaktionen
            if (udpScrollViewer != null)
            {
                udpScrollViewer.ScrollChanged += (s, e) =>
                {
                    // Wenn der Benutzer manuell scrollt (nicht durch Programmcode)
                    if (e.ExtentHeightChange == 0 && IsScrollingPossible(udpScrollViewer))
                    {
                        // Status auf Basis der aktuellen Position setzen
                        udpUserNotAtEnd = !IsAtEnd(udpScrollViewer);
                    }
                };
            }

            if (tcpScrollViewer != null)
            {
                tcpScrollViewer.ScrollChanged += (s, e) =>
                {
                    // Wenn der Benutzer manuell scrollt (nicht durch Programmcode)
                    if (e.ExtentHeightChange == 0 && IsScrollingPossible(tcpScrollViewer))
                    {
                        // Status auf Basis der aktuellen Position setzen
                        tcpUserNotAtEnd = !IsAtEnd(tcpScrollViewer);
                    }
                };
            }

            // Alternativ, falls ScrollViewer nicht funktioniert
            udpMonitorTextField.PreviewMouseWheel += (s, e) =>
            {
                if (IsScrollingPossible(udpScrollViewer ?? (ScrollViewer)null, udpMonitorTextField))
                {
                    UpdateUdpScrollStatus();
                }
            };

            tcpMonitorTextField.PreviewMouseWheel += (s, e) =>
            {
                if (IsScrollingPossible(tcpScrollViewer ?? (ScrollViewer)null, tcpMonitorTextField))
                {
                    UpdateTcpScrollStatus();
                }
            };

            // RemoteAccess-Instanz erzeugen
            remoteAccess = new RemoteAccess(UpdateUdpTextField, UpdateTcpTextField, ipAddress, udpPort, tcpPort, "udp", UpdateStartOptions, UpdateVariables, UpdateVariableInfo);

            // Fenster-Schliessen-Ereignis
            this.Closed += RemoteAccessWindow_Closed;
        }

        // Hilfsmethode zum Aktualisieren des UDP-Scroll-Status basierend auf der aktuellen Position
        private void UpdateUdpScrollStatus()
        {
            bool isAtEnd = IsAtEnd(udpScrollViewer, udpMonitorTextField);
            udpUserNotAtEnd = !isAtEnd;
        }

        // Hilfsmethode zum Aktualisieren des TCP-Scroll-Status basierend auf der aktuellen Position
        private void UpdateTcpScrollStatus()
        {
            bool isAtEnd = IsAtEnd(tcpScrollViewer, tcpMonitorTextField);
            tcpUserNotAtEnd = !isAtEnd;
        }

        // Hilfsmethode: Prüft, ob Scrollen überhaupt möglich ist (mehr Inhalt als sichtbar)
        private bool IsScrollingPossible(ScrollViewer scrollViewer, TextBox textBox = null)
        {
            if (scrollViewer != null)
            {
                return scrollViewer.ExtentHeight > scrollViewer.ViewportHeight;
            }
            else if (textBox != null)
            {
                return textBox.ExtentHeight > textBox.ViewportHeight;
            }

            return false;
        }

        // Hilfsmethode: Prüft, ob wir am Ende des scrollbaren Bereichs sind
        private bool IsAtEnd(ScrollViewer scrollViewer, TextBox textBox = null)
        {
            if (scrollViewer != null)
            {
                return Math.Abs(scrollViewer.VerticalOffset + scrollViewer.ViewportHeight - scrollViewer.ExtentHeight) < 1.0;
            }
            else if (textBox != null)
            {
                return Math.Abs(textBox.VerticalOffset + textBox.ViewportHeight - textBox.ExtentHeight) < 1.0;
            }

            return true; // Annahme: Wenn wir nicht scrollen können, sind wir am "Ende"
        }

        // Hilfsmethode zum Finden des ScrollViewers einer TextBox
        private ScrollViewer FindScrollViewer(TextBox textBox)
        {
            if (VisualTreeHelper.GetChildrenCount(textBox) == 0)
            {
                return null;
            }

            // Der ScrollViewer ist ein Kind des ersten visuellen Kindes der TextBox
            DependencyObject firstChild = VisualTreeHelper.GetChild(textBox, 0);
            if (firstChild == null)
            {
                return null;
            }

            // Suche nach dem ScrollViewer in der visuellen Hierarchie
            for (int i = 0; i < VisualTreeHelper.GetChildrenCount(firstChild); i++)
            {
                DependencyObject child = VisualTreeHelper.GetChild(firstChild, i);
                if (child is ScrollViewer scrollViewer)
                {
                    return scrollViewer;
                }
            }

            return null;
        }

        protected override async void OnContentRendered(EventArgs e)
        {
            base.OnContentRendered(e);

            // Nochmals versuchen, ScrollViewer zu bekommen, falls sie im Konstruktor noch nicht verfügbar waren
            if (udpScrollViewer == null)
                udpScrollViewer = FindScrollViewer(udpMonitorTextField);
            if (tcpScrollViewer == null)
                tcpScrollViewer = FindScrollViewer(tcpMonitorTextField);

            // UDP- und TCP-Listener-Tasks starten
            var udpListenerTask = remoteAccess.StartUdpListener();
            var tcpConnectTask = remoteAccess.ConnectTcp();

            await tcpConnectTask; // Wartet, bis TCP-Verbindung steht
            var tcpListenerTask = remoteAccess.StartTcpListener();

            // Auf beide warten
            await Task.WhenAll(udpListenerTask, tcpListenerTask);
        }

        private void RemoteAccessWindow_Closed(object sender, EventArgs e)
        {
            // Aufrufen der Dispose-Methode der RemoteAccess-Instanz
            remoteAccess.Dispose();
        }

        private void UpdateUdpTextField(string text)
        {
            if (string.IsNullOrEmpty(text))
                return;

            Dispatcher.Invoke(() =>
            {
                // Prüfen, ob wir am Ende sind und ob Scrollen überhaupt möglich ist
                bool canScroll = IsScrollingPossible(udpScrollViewer, udpMonitorTextField);

                // Die aktuelle Scroll-Position speichern
                double currentOffset = udpScrollViewer?.VerticalOffset ?? udpMonitorTextField.VerticalOffset;

                // Text hinzufügen
                udpMonitorTextField.AppendText(text);

                // Entscheidung, ob gescrollt werden soll
                if (!canScroll || !udpUserNotAtEnd)
                {
                    // Wenn nicht gescrollt werden kann ODER der Benutzer am Ende ist
                    // -> zum Ende scrollen
                    udpMonitorTextField.ScrollToEnd();
                }
                else
                {
                    // Der Benutzer ist nicht am Ende -> Position beibehalten
                    Dispatcher.BeginInvoke(DispatcherPriority.Loaded, new Action(() =>
                    {
                        if (udpScrollViewer != null)
                        {
                            udpScrollViewer.ScrollToVerticalOffset(currentOffset);
                        }
                        else
                        {
                            udpMonitorTextField.ScrollToVerticalOffset(currentOffset);
                        }
                    }));
                }
            });
        }

        private void UpdateTcpTextField(string text)
        {
            if (string.IsNullOrEmpty(text))
                return;

            Dispatcher.Invoke(() =>
            {
                // Prüfen, ob wir am Ende sind und ob Scrollen überhaupt möglich ist
                bool canScroll = IsScrollingPossible(tcpScrollViewer, tcpMonitorTextField);

                // Die aktuelle Scroll-Position speichern
                double currentOffset = tcpScrollViewer?.VerticalOffset ?? tcpMonitorTextField.VerticalOffset;

                // Text hinzufügen
                tcpMonitorTextField.AppendText(text);

                // Entscheidung, ob gescrollt werden soll
                if (!canScroll || !tcpUserNotAtEnd)
                {
                    // Wenn nicht gescrollt werden kann ODER der Benutzer am Ende ist
                    // -> zum Ende scrollen
                    tcpMonitorTextField.ScrollToEnd();
                }
                else
                {
                    // Der Benutzer ist nicht am Ende -> Position beibehalten
                    Dispatcher.BeginInvoke(DispatcherPriority.Loaded, new Action(() =>
                    {
                        if (tcpScrollViewer != null)
                        {
                            tcpScrollViewer.ScrollToVerticalOffset(currentOffset);
                        }
                        else
                        {
                            tcpMonitorTextField.ScrollToVerticalOffset(currentOffset);
                        }
                    }));
                }
            });
        }

        // Methode zum Zurücksetzen des Scroll-Status und Scrollen zum Ende
        public void ResetScrollState()
        {
            udpUserNotAtEnd = false;
            tcpUserNotAtEnd = false;
            udpMonitorTextField.ScrollToEnd();
            tcpMonitorTextField.ScrollToEnd();
        }

        private void UpdateStartOptions(List<string> options)
        {
            Dispatcher.Invoke(() =>
            {
                // UI-Elemente mit den Optionen aktualisieren
                StartOptionComboBox.ItemsSource = options;
            });
        }

        private void UpdateVariables(List<(string Name, UInt32 Value)> variables)
        {
            Dispatcher.Invoke(() =>
            {
                // Bestehende Variablen-Controls entfernen (falls vorhanden)
                var controlsToRemove = ControlStackPanel.Children
                    .OfType<FrameworkElement>()
                    .Where(element => element.Tag?.ToString() == "VariableControl")
                    .ToList();

                foreach (var control in controlsToRemove)
                {
                    ControlStackPanel.Children.Remove(control);
                }

                // Für jede Variable ein Label und eine TextBox hinzufügen
                foreach (var (name, value) in variables)
                {
                    // Horizontales StackPanel für die Zeile erstellen
                    StackPanel horizontalPanel = new StackPanel
                    {
                        Orientation = Orientation.Horizontal,
                        Margin = new Thickness(0, 5, 0, 0),
                        Tag = "VariableControl" // Tag zur Identifikation
                    };

                    // Label mit dem Variablennamen
                    Label label = new Label
                    {
                        Content = name,
                        Width = 150,
                        VerticalAlignment = VerticalAlignment.Center
                    };

                    // TextBox für uint32-Werte mit dem aktuellen Wert
                    TextBox textBox = new TextBox
                    {
                        Text = value.ToString(), // Hier wird der Wert gesetzt
                        Width = 100,
                        Margin = new Thickness(5, 0, 0, 0),
                        VerticalAlignment = VerticalAlignment.Center
                    };

                    // Input-Validierung für uint32
                    textBox.PreviewTextInput += (s, e) =>
                    {
                        // Nur Ziffern erlauben
                        e.Handled = !uint.TryParse(e.Text, out _);
                    };

                    // TextChanged-Handler für das Senden des aktualisierten Werts
                    textBox.PreviewKeyDown += (s, e) =>
                    {
                        if (e.Key == Key.Enter && s is TextBox tb && uint.TryParse(tb.Text, out uint newValue))
                        {
                            remoteAccess.SendVariable(name, (int)newValue);
                            e.Handled = true; // Verhindert das Standardverhalten der Enter-Taste
                        }
                    };

                    // Den Namen der Variablen als Tag speichern
                    textBox.Tag = name;

                    // Elemente zum horizontalen StackPanel hinzufügen
                    horizontalPanel.Children.Add(label);
                    horizontalPanel.Children.Add(textBox);

                    // Das horizontale Panel zum ControlStackPanel hinzufügen
                    ControlStackPanel.Children.Add(horizontalPanel);
                }
            });
        }

        private void UpdateVariableInfo((string Name, string Value) variableInfo)
        {
            Dispatcher.Invoke(() =>
            {
                // Nach einem vorhandenen Label mit dem gleichen Variablennamen suchen
                Label existingLabel = null;

                foreach (var child in VariableMonitorStackPanel.Children)
                {
                    if (child is Label label && label.Tag is string tagName && tagName == variableInfo.Name)
                    {
                        existingLabel = label;
                        break;
                    }
                }

                if (existingLabel != null)
                {
                    // Wenn eine bestehende Variable gefunden wurde, nur den Wert aktualisieren
                    existingLabel.Content = $"{variableInfo.Name}: {variableInfo.Value}";
                }
                else
                {
                    // Ansonsten ein neues Label hinzufügen
                    var newLabel = new Label
                    {
                        Content = $"{variableInfo.Name}: {variableInfo.Value}",
                        Margin = new Thickness(0, 0, 0, 5),
                        Tag = variableInfo.Name // Den Variablennamen als Tag speichern zur Identifikation
                    };

                    VariableMonitorStackPanel.Children.Add(newLabel);
                }
            });
        }


        private void SendStartOptionButton_Click(object sender, RoutedEventArgs e)
        {
            if (StartOptionComboBox.SelectedItem != null)
            {
                remoteAccess.SendStartOption();
            }
            else
            {
                MessageBox.Show("Bitte wählen Sie eine Option aus der Liste aus.", "Keine Option ausgewählt", MessageBoxButton.OK, MessageBoxImage.Warning);
            }
        }

        private void SendResetButton_Click(object sender, RoutedEventArgs e)
        {
            remoteAccess.SendReset();
        }

        private void StartOptionComboBox_DropDownClosed(object sender, EventArgs e)
        {
            remoteAccess.SelectedStartOption = StartOptionComboBox.SelectedItem as string;
        }

        private void AutoStartCheckBox_Click(object sender, RoutedEventArgs e)
        {
            if (remoteAccess != null)
            {
                remoteAccess.AutoStart = AutoStartCheckBox.IsChecked ?? false;
            }
        }


    }
}